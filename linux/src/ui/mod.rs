mod actions;
mod auxiliary;
mod chrome;
mod colors;
mod components;
mod composer;
mod connection;
mod events;
mod firmware_document;
mod flash;
mod highlights;
mod icons;
use components::{control_bg, native_button};
mod logs;
mod memory;
mod plot;
mod plot_surface;
mod polling;
mod profiles;
mod protocol;
mod records;
mod serial;
mod serial_forms;
mod settings;
mod state;
mod target;
mod terminal;
mod terminal_state;
mod terminal_table;
mod workflow;

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
    ActiveTheme, Disableable, Selectable, Sizable, WindowExt,
    button::*,
    checkbox::Checkbox,
    input::{EditorState, Input, InputState, Textarea, TextareaState},
    menu::{DropdownMenu, PopupMenuItem},
    resizable::{h_resizable, resizable_panel, v_resizable},
    theme::{Theme, ThemeMode},
};
use gpui_kit::prelude::FluentBuilder;
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
        CloseWindow,
        Find,
        Refresh,
        ConnectTarget,
        DisconnectTarget,
        SaveProfile,
        ExecutePrimary,
        ToggleAutoSend,
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
    focus_handle: FocusHandle,
    expanded: std::collections::BTreeSet<&'static str>,
    connected_target: Option<String>,
    worker: Worker,
    main_window: AnyWindowHandle,
    auxiliary: Vec<(auxiliary::Panel, AnyWindowHandle)>,
    busy_lanes: Vec<Lane>,
    installing: bool,
    preferences: Preferences,
    path: Option<PathBuf>,
    fields: BTreeMap<String, Entity<InputState>>,
    editors: BTreeMap<String, Entity<TextareaState>>,
    settings_tab: &'static str,
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
    terminal_rows: std::rc::Rc<Vec<terminal_table::Row>>,
    terminal_scroll: UniformListScrollHandle,
    terminal_ranges: BTreeMap<u64, std::ops::Range<usize>>,
    selected_record: Option<u64>,
    record_sort: usize,
    record_desc: bool,
    poll_pending: std::collections::HashSet<u64>,
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
            .track_focus(&self.focus_handle)
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
            .on_action(cx.listener(|s, _: &Refresh, w, cx| {
                if s.confirmation.is_none() && !s.is_busy() && !w.has_active_dialog(cx) {
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
            .on_action(cx.listener(|s, _: &ConnectTarget, w, cx| {
                if s.confirmation.is_some() || s.is_busy() || w.has_active_dialog(cx) {
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
                if s.confirmation.is_some() || s.is_busy() || w.has_active_dialog(cx) {
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
                if !s.is_busy() && !w.has_active_dialog(cx) {
                    s.confirmation = None;
                    s.open_save_profile(w, cx);
                }
            }))
            .on_action(cx.listener(|s, _: &ExecutePrimary, w, cx| {
                if s.confirmation.is_some() || s.is_busy() || w.has_active_dialog(cx) {
                    return;
                }
                if s.page == "serial" {
                    if !s.serial_connected() {
                        return;
                    }
                    if s.preferences.flag("command_mode") {
                        s.serial_action("command", cx);
                    } else if let Some(id) = s.active {
                        s.submit(Command::Send(id, s.snapshot(cx)), cx);
                    }
                } else if s.target_connected(cx) && !s.snapshot(cx).get("firmware").is_empty() {
                    s.submit(Command::Flash(s.snapshot(cx)), cx);
                }
            }))
            .on_action(cx.listener(|s, _: &ToggleAutoSend, w, cx| {
                if s.page == "serial" && !w.has_active_dialog(cx) {
                    s.toggle_auto_send(cx);
                }
            }))
            .on_action(cx.listener(|s, _: &ReadMemory, w, cx| {
                if s.confirmation.is_none()
                    && !s.is_busy()
                    && s.target_connected(cx)
                    && !w.has_active_dialog(cx)
                {
                    if let Err(e) = s.open_panel(auxiliary::Panel::Memory, cx) {
                        s.report_error(e.to_string(), cx);
                        return;
                    }
                    s.submit(Command::Memory(s.snapshot(cx)), cx);
                }
            }))
            .on_action(cx.listener(|s, _: &ClearTerminal, w, cx| {
                if s.page == "serial" {
                    s.clear_terminal(w, cx);
                }
            }))
            .on_action(cx.listener(|s, _: &ShowSettings, _, cx| {
                if let Err(e) = s.open_panel(auxiliary::Panel::Settings, cx) {
                    s.report_error(e.to_string(), cx);
                }
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
            .on_action(cx.listener(|_, _: &CloseWindow, w, cx| {
                if w.has_active_dialog(cx) {
                    w.close_dialog(cx);
                } else {
                    w.dispatch_action(Box::new(Quit), cx);
                }
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
                if let Err(e) = s.open_panel(auxiliary::Panel::Logs, cx) {
                    s.report_error(e.to_string(), cx);
                }
            }))
            .on_action(cx.listener(|s, _: &ShowMemory, _, cx| {
                if let Err(e) = s.open_panel(auxiliary::Panel::Memory, cx) {
                    s.report_error(e.to_string(), cx);
                }
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
            .text_size(px(13.))
            .line_height(relative(1.2))
            .child(self.toolbar(window, cx))
            .child(if matches!(self.page, "flash" | "serial") {
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(body)
                    .into_any_element()
            } else {
                div()
                    .id("content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_6()
                    .child(body)
                    .into_any_element()
            })
    }
}
