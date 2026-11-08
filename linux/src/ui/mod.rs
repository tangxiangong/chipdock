mod actions;
mod auxiliary;
mod components;
mod events;
mod flash;
mod highlights;
mod logs;
mod memory;
mod plot;
mod polling;
mod profiles;
mod serial;
mod settings;
mod state;
mod terminal;
mod terminal_state;

use crate::{
    backend::{Command, Lane, Output, Worker, memory_text},
    model::{self, Preferences, Profile},
};
use anyhow::{Context as _, Result};
use chipdock_core::{
    models::*,
    serial::{Direction, SerialFrameEvent, types::*},
};
use chipdock_service::serial as service;
use gpui_kit::component::{
    ActiveTheme, Disableable, Selectable, Sizable,
    button::*,
    checkbox::Checkbox,
    input::{Editor, EditorState, Input, InputState, Textarea, TextareaState},
    theme::{Theme, ThemeMode},
};
use gpui_kit::*;
use serde_json::{Value, json};
gpui_kit::actions!(
    chipdock,
    [
        OpenFirmware,
        ShowLogs,
        ShowMemory,
        SaveSettings,
        ShowFlash,
        ShowSerial,
        Quit,
        Find,
        Refresh,
        ConnectTarget,
        DisconnectTarget,
        SaveProfile,
        ExecutePrimary,
        ReadMemory,
        ClearTerminal,
        ShowSettings
    ]
);
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::mpsc,
    time::Duration,
};

struct Session {
    id: u64,
    cursor: u64,
    cleared_through: u64,
    frames: VecDeque<SerialFrameEvent>,
    status: Option<SerialStatus>,
    preferences: Preferences,
    plot_points: Vec<Value>,
    plot_cursor: u64,
    plot_revision: u64,
    plot_dropped: u64,
    plot_config: String,
    plotting: bool,
    paused: bool,
    result: String,
}
pub struct Workbench {
    worker: Worker,
    main_window: AnyWindowHandle,
    auxiliary: Vec<(auxiliary::Panel, AnyWindowHandle)>,
    busy_lanes: Vec<Lane>,
    installing: bool,
    preferences: Preferences,
    path: Option<PathBuf>,
    fields: BTreeMap<String, Entity<InputState>>,
    editors: BTreeMap<String, Entity<TextareaState>>,
    page: &'static str,
    serial_page: &'static str,
    message: String,
    logs: VecDeque<String>,
    unseen_errors: usize,
    logs_scroll: ScrollHandle,
    output: Entity<TextareaState>,
    firmware_output: Entity<TextareaState>,
    memory_output: Entity<TextareaState>,
    terminal: Entity<EditorState>,
    probes: Vec<ProbeSummary>,
    chips: Vec<String>,
    ports: Vec<PortInfo>,
    memory: Option<MemoryReadResult>,
    preview: Option<FirmwarePreview>,
    firmware_key: Option<(String, String)>,
    layout: Vec<MemoryRegionLayout>,
    sessions: Vec<Session>,
    active: Option<u64>,
    paused: bool,
    terminal_style: String,
    terminal_ranges: BTreeMap<u64, std::ops::Range<usize>>,
    selected_record: Option<u64>,
    poll_pending: std::collections::HashSet<u64>,
    next_port_scan: std::time::Instant,
    poll_tx: mpsc::Sender<(u64, u64)>,
    poll_rx: mpsc::Receiver<(u64, Result<chipdock_core::serial::PollBatch>)>,
    update: Option<crate::update::Available>,
    downloaded: Option<PathBuf>,
    next_update_check: std::time::Instant,
    confirmation: Option<Preferences>,
    plot_points: Vec<Value>,
    plot_cursor: u64,
    plot_revision: u64,
    plot_dropped: u64,
    plot_config: String,
    plotting: bool,
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.page {
            "flash" => self.flash_page(window, cx),
            "memory" => self.memory_page(cx),
            "serial" => self.serial_page(window, cx),
            "profiles" => self.profiles_page(cx),
            "settings" => self.settings_page(cx),
            _ => self.logs_page(cx),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .on_action(cx.listener(|s, _: &Find, w, cx| {
                let key = if s.page == "logs" {
                    "log_search"
                } else {
                    "filter"
                };
                if s.page != "logs" {
                    s.page = "serial";
                    s.serial_page = "terminal";
                }
                s.fields[key].update(cx, |field, cx| field.focus(w, cx));
                cx.notify();
            }))
            .on_action(cx.listener(|s, _: &Refresh, _, cx| {
                if s.confirmation.is_none() {
                    s.submit(
                        if s.page == "serial" {
                            Command::Ports
                        } else {
                            Command::Scan
                        },
                        cx,
                    );
                }
            }))
            .on_action(cx.listener(|s, _: &ConnectTarget, _, cx| {
                if s.confirmation.is_some() {
                    return;
                }
                if s.page == "serial" {
                    if let Some(id) = s.active {
                        let connected = s
                            .sessions
                            .iter()
                            .find(|s| s.id == id)
                            .and_then(|s| s.status.as_ref())
                            .is_some_and(|s| s.connected || s.reconnecting);
                        s.submit(
                            if connected {
                                Command::Disconnect(id)
                            } else {
                                Command::ConnectSerial(id, s.snapshot(cx))
                            },
                            cx,
                        );
                    }
                } else {
                    s.submit(Command::Connect(s.snapshot(cx)), cx);
                }
            }))
            .on_action(cx.listener(|s, _: &DisconnectTarget, w, cx| {
                if s.confirmation.is_some() {
                    return;
                }
                if s.page == "serial" {
                    if let Some(id) = s.active {
                        s.submit(Command::Disconnect(id), cx);
                    }
                } else if !s.is_busy() {
                    s.disconnect_target(w, cx);
                }
            }))
            .on_action(cx.listener(|s, _: &SaveProfile, w, cx| {
                if !s.is_busy() {
                    s.confirmation = None;
                    s.page = "profiles";
                    s.fields["profile"].update(cx, |s, cx| s.focus(w, cx));
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|s, _: &ExecutePrimary, _, cx| {
                if s.confirmation.is_some() {
                    return;
                }
                if s.page == "serial" {
                    if s.serial_page == "protocol" {
                        s.serial_action("command", cx);
                    } else if let Some(id) = s.active {
                        s.submit(Command::Send(id, s.snapshot(cx)), cx);
                    }
                } else {
                    s.submit(Command::Flash(s.snapshot(cx)), cx);
                }
            }))
            .on_action(cx.listener(|s, _: &ReadMemory, _, cx| {
                if s.confirmation.is_none() {
                    s.page = "memory";
                    s.submit(Command::Memory(s.snapshot(cx)), cx);
                }
            }))
            .on_action(cx.listener(|s, _: &ClearTerminal, w, cx| {
                if s.page == "serial" {
                    s.clear_terminal(w, cx);
                }
            }))
            .on_action(cx.listener(|s, _: &ShowSettings, _, cx| {
                s.confirmation = None;
                s.page = "settings";
                cx.notify();
            }))
            .on_action(cx.listener(|s, _: &ShowFlash, _, cx| {
                s.page = "flash";
                cx.notify();
            }))
            .on_action(cx.listener(|s, _: &ShowSerial, _, cx| {
                s.confirmation = None;
                s.page = "serial";
                cx.notify();
            }))
            .on_action(cx.listener(|s, _: &Quit, w, cx| {
                if s.busy_lanes.iter().all(|l| l.background())
                    && s.sessions.iter().all(|v| {
                        v.status
                            .as_ref()
                            .is_none_or(|v| v.job_kind.is_none() && !v.logging && !v.capturing)
                    })
                {
                    s.save(w, cx);
                    s.close_panels(cx);
                    w.remove_window();
                } else {
                    s.report("请先停止任务和采集，再退出", cx);
                }
            }))
            .on_action(cx.listener(|s, _: &OpenFirmware, _, cx| s.pick("firmware", cx)))
            .on_action(cx.listener(|s, _: &ShowLogs, _, cx| {
                s.confirmation = None;
                s.unseen_errors = 0;
                s.page = "logs";
                cx.notify();
            }))
            .on_action(cx.listener(|s, _: &ShowMemory, _, cx| {
                s.confirmation = None;
                s.page = "memory";
                cx.notify();
            }))
            .on_action(cx.listener(|s, _: &SaveSettings, w, cx| s.save(w, cx)))
            .on_drop(cx.listener(|s, paths: &ExternalPaths, w, cx| {
                if !s.is_busy()
                    && let Some(path) = paths.paths().first()
                {
                    s.set("firmware", path.to_string_lossy().into_owned(), w, cx);
                    s.page = "flash";
                    s.submit(Command::Preview(s.snapshot(cx)), cx);
                }
            }))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .h(px(48.))
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("ChipDock"))
                    .child(div().text_sm().child(self.message.clone())),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .w(px(168.))
                            .flex_shrink_0()
                            .p_3()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .bg(cx.theme().sidebar)
                            .children(
                                [
                                    ("flash", "固件烧录"),
                                    ("serial", "串口调试"),
                                    ("memory", "目标内存"),
                                    ("profiles", "配置与历史"),
                                    ("logs", "操作日志"),
                                    ("settings", "设置"),
                                ]
                                .into_iter()
                                .map(|(page, label)| {
                                    Button::new(page)
                                        .label(if page == "logs" && self.unseen_errors > 0 {
                                            format!("{label} ({})", self.unseen_errors)
                                        } else {
                                            label.to_owned()
                                        })
                                        .selected(self.page == page)
                                        .on_click(cx.listener(move |s, _, _, cx| {
                                            s.confirmation = None;
                                            if page == "logs" {
                                                s.unseen_errors = 0;
                                            }
                                            s.page = page;
                                            cx.notify();
                                        }))
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("content")
                            .flex_1()
                            .min_w_0()
                            .overflow_y_scroll()
                            .p_6()
                            .child(body),
                    ),
            )
    }
}
