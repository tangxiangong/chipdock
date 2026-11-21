use super::*;
use chipdock_core::serial::{Direction, types::ControlLine};
use nsh_base::{
    DataTable, DataTableApi, DataTableCell, DataTableClasses, DataTableColumn,
    DataTableContextMenu, DataTableEvent, DataTablePress, DataTableRange, DataTableRows,
    DataTableSelection, SelectItem,
};
use nsh_core::element::desktop::MenuItem as NativeMenuItem;
use std::collections::BTreeMap;

#[derive(Clone, PartialEq)]
struct Record {
    id: u64,
    time: String,
    direction: &'static str,
    bytes: usize,
    text: String,
    color: Option<u32>,
    json: String,
}

#[derive(Clone, Default)]
pub(super) struct TerminalState {
    pub selected: Vec<u64>,
    pub anchor: Option<u64>,
    pub widths: BTreeMap<String, f32>,
    pub top_sequence: Option<u64>,
}

pub(super) fn navigate(store: Store, next: bool) {
    let ids = store.read(|s| {
        s.active_session()
            .map(|session| {
                crate::presentation::filtered(session.records(), &s.preferences)
                    .unwrap_or_default()
                    .iter()
                    .map(|r| r.sequence)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    if let Some(id) = next_sequence(&ids, store.jump.get_untracked(), next) {
        store.change(|s| s.flag("auto_scroll", false));
        store.jump.set(Some(id));
    }
}

fn next_sequence(ids: &[u64], current: Option<u64>, next: bool) -> Option<u64> {
    if ids.is_empty() {
        return None;
    }
    let at = current.and_then(|id| ids.iter().position(|row| *row == id));
    let index = match (at, next) {
        (None, true) => 0,
        (None, false) => ids.len() - 1,
        (Some(index), true) => (index + 1) % ids.len(),
        (Some(index), false) => (index + ids.len() - 1) % ids.len(),
    };
    ids.get(index).copied()
}

fn select_rows(
    ids: &[u64],
    selected: &[u64],
    anchor: Option<u64>,
    row: usize,
    shift: bool,
    toggle: bool,
) -> (Vec<u64>, Option<u64>) {
    let Some(id) = ids.get(row).copied() else {
        return (selected.to_vec(), anchor);
    };
    if shift {
        let start = anchor
            .and_then(|id| ids.iter().position(|v| *v == id))
            .unwrap_or(row);
        let range = &ids[start.min(row)..=start.max(row)];
        let mut next = if toggle {
            selected.to_vec()
        } else {
            Vec::new()
        };
        for id in range {
            if !next.contains(id) {
                next.push(*id)
            }
        }
        (next, anchor.or(Some(id)))
    } else if toggle {
        let mut next = selected.to_vec();
        if let Some(at) = next.iter().position(|v| *v == id) {
            next.remove(at);
        } else {
            next.push(id)
        }
        (next, Some(id))
    } else {
        (vec![id], Some(id))
    }
}

pub(super) fn session_bar(store: Store) -> View {
    let sessions = computed(move || store.read(|s| s.sessions.keys().copied().collect::<Vec<_>>()));
    view()
        .h(px(33.))
        .px(px(10.))
        .flex_row()
        .items_center()
        .gap_2()
        .border_b_1()
        .border_color(hairline)
        .child(
            scroll()
                .horizontal()
                .flex_1()
                .child(view().flex_row().gap_1().child(each(
                    sessions,
                    |id: &u64| *id,
                    move |id, _| {
                        let tab = view()
                            .flex_row()
                            .items_center()
                            .child(
                                nsh_base::checkbox()
                                    .checked(move || store.read(|s| s.active == Some(id.get())))
                                    .indicator_style(|style| style.hidden())
                                    .on_change(move |checked: &bool| {
                                        if *checked {
                                            store.change(|s| s.switch_session(id.get_untracked()));
                                        }
                                    })
                                    .h(px(20.))
                                    .px(px(10.))
                                    .rounded(move || theme().radius)
                                    .flex_row()
                                    .items_center()
                                    .gap(px(5.))
                                    .checked_style(|s| {
                                        s.bg(move || {
                                            theme()
                                                .color("session-selected")
                                                .unwrap_or(theme().secondary)
                                        })
                                    })
                                    .hover(|s| s.bg(move || theme().accent))
                                    .active(|s| s.bg(move || theme().muted))
                                    .focus_visible(|s| {
                                        s.outline(px(2.)).outline_color(move || theme().ring)
                                    })
                                    .child(view().size(px(6.)).rounded_full().bg(move || {
                                        store.read(|s| {
                                            if s.sessions
                                                .get(&id.get())
                                                .is_some_and(|s| s.connected())
                                            {
                                                theme().color("success").unwrap_or(theme().primary)
                                            } else {
                                                theme().muted_foreground.opacity(0.4)
                                            }
                                        })
                                    }))
                                    .child(
                                        text(move || {
                                            store.read(|s| {
                                                s.sessions
                                                    .get(&id.get())
                                                    .and_then(|s| s.status.as_ref())
                                                    .and_then(|s| s.path.clone())
                                                    .unwrap_or_else(|| format!("会话 {}", id.get()))
                                            })
                                        })
                                        .text_size(small_control_size),
                                    ),
                            )
                            .child(when(
                                move || id.get() != 1 && store.read(|s| s.active == Some(id.get())),
                                move || {
                                    icon_button(
                                        "close",
                                        "关闭会话",
                                        || false,
                                        move || store.read(Workbench::serial_busy),
                                        move || store.change(Workbench::close_session),
                                    )
                                },
                            ));
                        tab.on_context_menu(move |event: &ContextMenuEvent| {
                            if id.get_untracked() != 1 {
                                drop(nsh_core::element::desktop::popup_menu_at(
                                    vec![
                                        NativeMenuItem::new("关闭会话")
                                            .disabled(store.read(|s| s.lane_busy(Lane::Serial)))
                                            .on_select(move || {
                                                store.command(|_| {
                                                    Command::CloseSession(id.get_untracked())
                                                })
                                            }),
                                    ],
                                    event.page_position,
                                ));
                            }
                        })
                    },
                ))),
        )
        .child(native_borderless_menu_small(
            view()
                .w(px(83.))
                .h(px(14.))
                .px(px(4.25))
                .flex_row()
                .items_center()
                .gap_1()
                .hit_slop(nsh_core::geometry::Edges::all(px(3.)))
                .text_color(move || {
                    theme()
                        .color("terminal-menu-glyph")
                        .unwrap_or(theme().foreground)
                })
                .label("本地控制")
                .child(icons::icon("terminal"))
                .child(text("本地控制").text_size(small_control_size))
                .child(text("⌄").text_size(small_size)),
            move || {
                [(0, "关闭"), (1, "只读"), (2, "允许控制")]
                    .into_iter()
                    .map(|(mode, label)| {
                        NativeMenuItem::new(label)
                            .checked(store.read(|s| s.preferences.ipc == mode))
                            .on_select(move || store.command(|_| Command::Ipc(mode)))
                    })
                    .collect()
            },
        ))
        .child(view().w(px(1.)).h_4().bg(hairline))
        .child(
            nsh_base::button()
                .accessibility_label("新建串口会话")
                .disabled(move || {
                    store.read(|state| state.lane_busy(Lane::Serial) || state.sessions.len() >= 16)
                })
                .on_press(move || {
                    store.command(|state| Command::NewSession(state.preferences.clone()))
                })
                .w(px(12.))
                .h(px(20.))
                .p_0()
                .items_center()
                .justify_center()
                .hit_slop(nsh_core::geometry::Edges::all(px(5.)))
                .text_color(move || {
                    theme()
                        .color("terminal-glyph")
                        .unwrap_or(theme().muted_foreground)
                })
                .focus_visible(|style| style.outline(px(2.)).outline_color(move || theme().ring))
                .child(icons::icon("plus").size(px(14.))),
        )
}

/// The active serial session's form state, projected without sending terminal records to JS.
#[derive(Debug, Clone, Default, PartialEq, JsObject)]
pub struct SerialFormState {
    pub command_mode: bool,
    pub send_characters: u32,
    pub send_job: bool,
    pub active_window: bool,
    pub baud_valid: bool,
    pub ports_busy: bool,
    pub options_locked: bool,
    pub auto_connect_locked: bool,
    pub auto_connect: bool,
    pub reconnect: bool,
    pub control_lines: bool,
    pub rts: bool,
    pub dtr: bool,
    pub rts_locked: bool,
    pub dtr_locked: bool,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub enum SerialPageEvent {
    /// The active session's serial form values or available controls changed.
    #[args(state)]
    FormChange(SerialFormState),
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct SerialPageProps {
    /// The connection, terminal and composer regions arranged by the page.
    pub children: Prop<Children>,
}

/// Commands from the connection form and composer tabs.
pub struct SerialPageApi {
    store: Store,
}

#[methods]
impl SerialPageApi {
    /// Requests a fresh enumeration of serial ports.
    pub fn refresh_ports(&self) {
        if !self.store.read(|s| s.lane_busy(Lane::Serial)) {
            self.store.command(|_| Command::Ports);
        }
    }
    /// Switches the active session between its text and command editors.
    pub fn set_command_mode(&self, command_mode: bool) {
        self.store.change(|s| s.flag("command_mode", command_mode));
    }
    /// Applies the default session's start-up connection preference.
    pub fn set_auto_connect(&self, enabled: bool) {
        if !self.store.locked("port") && self.store.read(|s| s.active == Some(1)) {
            self.store.change(|s| s.flag("auto_connect", enabled));
        }
    }
    /// Applies the active session's reconnection preference.
    pub fn set_reconnect(&self, enabled: bool) {
        if !self.store.locked("port") {
            self.store.change(|s| s.flag("reconnect", enabled));
        }
    }
    /// Changes an output line on the connected physical port.
    pub fn set_line(&self, line: SerialOutputLine, level: bool) {
        let Some(id) = self.store.read(|s| {
            let status = s.active_session()?.status.as_ref()?;
            (status.connected
                && !status.path.as_deref().unwrap_or("").starts_with("VIRTUAL-")
                && !s.serial_busy()
                && !(line == SerialOutputLine::Rts && s.preferences.get("flow") == "hardware"))
                .then_some(s.active?)
        }) else {
            return;
        };
        self.store.command(|_| {
            Command::Line(
                id,
                match line {
                    SerialOutputLine::Rts => ControlLine::Rts,
                    SerialOutputLine::Dtr => ControlLine::Dtr,
                },
                level,
            )
        });
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PropEnum)]
pub enum SerialOutputLine {
    #[default]
    Rts,
    Dtr,
}

/// Binds the serial page to the workbench's active session.
#[component(methods = SerialPageApi)]
pub fn SerialPageHost(props: SerialPageProps, cx: Scope<SerialPageEvent>) -> impl IntoElement {
    let (store, _) = workbench_context();
    cx.methods(SerialPageApi { store });
    let active = control_active();
    let state = computed(move || {
        let active_window = active.get();
        let options_locked = store.locked("port");
        store.read(|s| {
            let status = s
                .active_session()
                .and_then(|session| session.status.as_ref());
            SerialFormState {
                command_mode: s.preferences.flag("command_mode"),
                send_characters: s.preferences.get("send").chars().count() as u32,
                send_job: status
                    .and_then(|v| v.job_kind.as_deref())
                    .is_some_and(|k| k != "repeat"),
                active_window,
                baud_valid: s
                    .preferences
                    .get("baud")
                    .trim()
                    .parse::<u32>()
                    .is_ok_and(|v| v > 0),
                ports_busy: s.lane_busy(Lane::Serial),
                options_locked,
                auto_connect_locked: options_locked || s.active != Some(1),
                auto_connect: s.preferences.flag("auto_connect"),
                reconnect: s.preferences.flag("reconnect"),
                control_lines: status.is_some_and(|v| {
                    v.connected && !v.path.as_deref().unwrap_or("").starts_with("VIRTUAL-")
                }),
                rts: status.and_then(|v| v.rts).unwrap_or(false),
                dtr: status.and_then(|v| v.dtr).unwrap_or(false),
                rts_locked: s.serial_busy() || s.preferences.get("flow") == "hardware",
                dtr_locked: s.serial_busy(),
            }
        })
    });
    effect(move || cx.emit(SerialPageEvent::FormChange(state.get())));
    view().child(props.children)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PropEnum)]
pub enum SerialChoiceField {
    #[default]
    Bits,
    Stop,
    Parity,
    Flow,
    Format,
    Ending,
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct SerialChoiceProps {
    /// Session preference edited by the native menu.
    pub field: Prop<SerialChoiceField>,
}

/// The shared native picker for a serial parameter or send format.
#[component]
pub fn SerialChoice(props: SerialChoiceProps) -> impl IntoElement {
    let (store, _) = workbench_context();
    let (key, choices): (&str, &'static [&'static str]) = match props.field.get() {
        SerialChoiceField::Bits => ("bits", &["5", "6", "7", "8"]),
        SerialChoiceField::Stop => ("stop", &["1", "2"]),
        SerialChoiceField::Parity => ("parity", &["none", "even", "odd"]),
        SerialChoiceField::Flow => ("flow", &["none", "software", "hardware"]),
        SerialChoiceField::Format => ("format", &["hex", "ascii", "gbk", "utf-8", "latin-1"]),
        SerialChoiceField::Ending => ("ending", &["none", "lf", "cr", "crlf"]),
    };
    popup_control(store, key, choices, true)
}

/// Baud rate editor with its native preset menu.
#[component]
pub fn SerialBaud() -> impl IntoElement {
    let (store, _) = workbench_context();
    view()
        .flex_1()
        .flex_row()
        .items_center()
        .justify_end()
        .gap(px(2.))
        .child(
            nsh_base::input()
                .value(move || store.field("baud"))
                .on_change(move |v: &str| store.change(|s| s.set("baud", v.into())))
                .label("波特率")
                .readonly(move || store.locked("baud"))
                .w(px(135.))
                .h(px(20.))
                .text_right()
                .text_size(small_control_size),
        )
        .child(native_borderless_menu_small_disabled(
            view()
                .w(px(17.))
                .h(px(16.))
                .items_center()
                .justify_center()
                .label("常用波特率")
                .child(icons::icon("select").size(px(12.))),
            move || store.locked("baud"),
            move || {
                [
                    1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
                ]
                .into_iter()
                .map(move |rate| {
                    NativeMenuItem::new(rate.to_string()).on_select(move || {
                        if !store.locked("baud") {
                            store.change(|s| s.set("baud", rate.to_string()));
                        }
                    })
                })
                .collect()
            },
        ))
}

/// Connect, disconnect or cancel reconnect according to the current serial state.
#[component]
pub fn SerialConnectButton() -> impl IntoElement {
    let (store, _) = workbench_context();
    let connect_disabled = computed(move || {
        store.read(|s| {
            s.serial_busy()
                || (!s.serial_connected()
                    && !s
                        .active_session()
                        .and_then(|v| v.status.as_ref())
                        .is_some_and(|v| v.reconnecting)
                    && (s.preferences.get("port").is_empty()
                        || !s
                            .preferences
                            .get("baud")
                            .trim()
                            .parse::<u32>()
                            .is_ok_and(|v| v > 0)))
        })
    });

    let active = control_active();
    nsh_base::button()
        .disabled(connect_disabled)
        .on_press(move || {
            if !store.modal.get_untracked() {
                store.change(Workbench::connect_serial);
            }
        })
        .h(px(28.))
        .px(px(14.))
        .rounded(px(14.))
        .flex_row()
        .items_center()
        .justify_center()
        .gap_2()
        .bg(move || emphasized_fill(true, connect_disabled.get(), active.get(), false))
        .text_color(move || emphasized_ink(true, connect_disabled.get(), active.get(), false))
        .hover(|s| s.bg(move || theme().primary.opacity(0.9)))
        .active(|s| s.bg(move || theme().primary.opacity(0.8)))
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .disabled_style(|s| s.opacity(1.))
        .child(
            when(
                move || store.read(Workbench::serial_connected),
                || icons::icon("stop"),
            )
            .otherwise(|| icons::icon("link")),
        )
        .child(
            text(move || {
                store.read(|s| {
                    if s.serial_busy() {
                        "正在处理…"
                    } else if s
                        .active_session()
                        .and_then(|v| v.status.as_ref())
                        .is_some_and(|v| v.reconnecting)
                    {
                        "取消重连"
                    } else if s.serial_connected() {
                        "断开连接"
                    } else {
                        "连接串口"
                    }
                })
            })
            .text_size(body_size),
        )
}

/// Native serial tool menu with current-session actions.
#[component]
pub fn SerialToolsMenu() -> impl IntoElement {
    let (store, _) = workbench_context();
    let repeating = move || repeating(store);
    native_borderless_menu_small(
        view()
            .w(px(58.))
            .h(px(14.))
            .px(px(3.))
            .flex_row()
            .items_center()
            .gap(px(4.))
            .hit_slop(nsh_core::geometry::Edges::all(px(3.)))
            .text_color(move || {
                theme()
                    .color("terminal-menu-glyph")
                    .unwrap_or(theme().foreground)
            })
            .label("串口工具")
            .child(icons::icon("more").size(px(14.)))
            .child(text("工具").text_size(small_control_size))
            .child(icons::icon("down").size(px(8.))),
        move || {
            vec![
                NativeMenuItem::new("命令与接收规则…")
                    .disabled(repeating())
                    .on_select(move || tools::open(store, "protocol")),
                NativeMenuItem::separator(),
                NativeMenuItem::new("校验、分帧、Modbus 与采集…")
                    .on_select(move || tools::open(store, "checksum")),
                NativeMenuItem::new("包库、文件、宏与应答…")
                    .on_select(move || tools::open(store, "packets")),
                NativeMenuItem::separator(),
                NativeMenuItem::new("打开实时曲线").on_select(move || {
                    if let Some(id) = store.read(|s| s.active) {
                        auxiliary::open(store, auxiliary::Panel::Plot(id));
                    }
                }),
            ]
        },
    )
}

/// Native menu for restoring a previously sent draft.
#[component]
pub fn SerialSendHistory() -> impl IntoElement {
    let (store, _) = workbench_context();
    let repeating = move || repeating(store);
    native_borderless_menu_small_disabled(
        view().size(px(20.)).label("发送历史").child(
            icons::icon("history").size(px(15.)).text_color(move || {
                button_ink(
                    false,
                    repeating() || store.read(|s| s.preferences.history.is_empty()),
                )
            }),
        ),
        move || repeating() || store.read(|s| s.preferences.history.is_empty()),
        move || {
            store
                .read(|s| s.preferences.history.clone())
                .into_iter()
                .map(|value| {
                    NativeMenuItem::new(value.chars().take(80).collect::<String>()).on_select(
                        move || {
                            if !repeating() {
                                store.change(|s| s.set("send", value.clone()));
                            }
                        },
                    )
                })
                .collect()
        },
    )
}

/// Multiline editor bound directly to the active session draft.
#[component]
pub fn SerialTextDraft() -> impl IntoElement {
    let (store, _) = workbench_context();
    let repeating = move || repeating(store);
    nsh_base::input()
        .value(move || store.field("send"))
        .on_change(move |v: &str| store.change(|s| s.set("send", v.into())))
        .label("要发送的数据")
        .multiline(true)
        .readonly(move || repeating() || store.read(|s| s.installing))
        .placeholder(move || {
            if store.field("format") == "hex" {
                "输入十六进制字节，例如 AA 01 FF"
            } else {
                "输入文本或设备指令…"
            }
        })
        .placeholder_color(|| color(theme().foreground.opacity(0.26)))
        .font_family(mono)
        .text_size(body_size)
        .px(px(14.))
        .py(px(6.))
        .flex_1()
        .min_h_0()
}

/// Repeat-send control with interval validation and cancellation.
#[component]
pub fn SerialRepeat() -> impl IntoElement {
    let (store, _) = workbench_context();
    let repeating = move || repeating(store);
    nsh_base::checkbox()
        .checked(repeating)
        .indicator_style(|style| style.hidden())
        .disabled(move || {
            !repeating()
                && store.read(|s| {
                    !super::commands::can_send(s)
                        || !s
                            .preferences
                            .number("interval")
                            .is_ok_and(|v| (10..=3_600_000).contains(&v))
                })
        })
        .on_change(move |_: &bool| {
            let stop = repeating();
            store.change(|s| s.serial_action(if stop { "cancelJob" } else { "repeat" }));
        })
        .h(px(20.))
        .px_2()
        .rounded(px(5.))
        .flex_row()
        .items_center()
        .gap(px(5.))
        .bg(move || theme().color("button").unwrap_or(theme().secondary))
        .border_1()
        .border_color(hairline)
        .checked_style(|s| {
            s.bg(move || theme().primary)
                .text_color(move || theme().primary_foreground)
        })
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().muted))
        .disabled_style(|s| s.opacity(0.45))
        .child(icons::icon("repeat").size(px(13.)))
        .child(text("自动发送").text_size(caption_size))
}

/// Live progress and cancellation for a non-repeating send job.
#[component]
pub fn SerialSendProgress() -> impl IntoElement {
    let (store, _) = workbench_context();
    let busy = move || store.read(Workbench::serial_busy);
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            nsh_base::progress()
                .value(move || {
                    store.read(|s| {
                        s.active_session()
                            .and_then(|s| s.status.as_ref())
                            .map_or(0., |s| {
                                if s.job_total > 0 {
                                    s.job_done as f32 / s.job_total as f32 * 100.
                                } else {
                                    0.
                                }
                            })
                    })
                })
                .w(px(80.))
                .track_class("h-1 bg-muted rounded-full")
                .indicator_class("h-1 bg-primary rounded-full")
                .accessibility_label("发送任务进度"),
        )
        .child(caption(move || {
            store.read(|s| {
                s.active_session()
                    .and_then(|s| s.status.as_ref())
                    .map_or(String::new(), |s| {
                        format!("{} / {}", s.job_done, s.job_total)
                    })
            })
        }))
        .child(action("取消任务", Variant::Default, busy, move || {
            store.change(|s| s.serial_action("cancelJob"))
        }))
}

/// Submit the active session draft after checking its send guards.
#[component]
pub fn SerialSendButton() -> impl IntoElement {
    let (store, _) = workbench_context();
    let repeating = move || repeating(store);
    let busy = move || store.read(Workbench::serial_busy);
    prominent_button(
        "send",
        move || {
            if busy() && store.read(Workbench::serial_connected) {
                "发送中…"
            } else {
                "发送"
            }
        },
        move || !store.read(super::commands::can_send) || repeating(),
        move || {
            if !store.modal.get_untracked() {
                store.change(Workbench::send);
            }
        },
    )
}

/// Native checksum policy menu for send operations.
#[component]
pub fn SerialChecksum() -> impl IntoElement {
    let (store, _) = workbench_context();
    checksum_menu(store)
}

/// The active session’s validated repeat interval editor.
#[component]
pub fn SerialInterval() -> impl IntoElement {
    let (store, _) = workbench_context();
    compact_input(store, "interval", "自动发送间隔，毫秒", 64.)
}

/// Command editor with completion, preview and keyboard history.
#[component]
pub fn SerialCommandEditor() -> impl IntoElement {
    let (store, _) = workbench_context();
    command_composer(store)
}

/// Searchable serial port selector with disconnected device reconciliation.
#[component]
pub fn SerialPortPicker() -> impl IntoElement {
    let (store, _) = workbench_context();
    device_picker(store)
}

/// Serial sessions with stable identities and native context menus.
#[component]
pub fn SerialSessions() -> impl IntoElement {
    let (store, _) = workbench_context();
    session_bar(store)
}

/// Virtualized receive records and native selection.
#[component]
pub fn SerialTerminal() -> impl IntoElement {
    let (store, _) = workbench_context();
    terminal(store)
}

/// Error reported by the active serial session.
#[component]
pub fn SerialError() -> impl IntoElement {
    let (store, _) = workbench_context();
    error_banner(store)
}

/// Live native serial telemetry and command controls.
#[component]
pub fn SerialStatus() -> impl IntoElement {
    let (store, _) = workbench_context();
    status_bar(store)
}

pub(super) fn register(registry: &mut Registry) {
    registry
        .register::<SerialPageHost>()
        .register::<SerialChoice>()
        .register::<SerialBaud>()
        .register::<SerialConnectButton>()
        .register::<SerialToolsMenu>()
        .register::<SerialSendHistory>()
        .register::<SerialTextDraft>()
        .register::<SerialRepeat>()
        .register::<SerialSendProgress>()
        .register::<SerialSendButton>()
        .register::<SerialChecksum>()
        .register::<SerialInterval>()
        .register::<SerialCommandEditor>()
        .register::<SerialPortPicker>()
        .register::<SerialSessions>()
        .register::<SerialTerminal>()
        .register::<SerialError>()
        .register::<SerialStatus>();
}

fn repeating(store: Store) -> bool {
    store.read(|s| {
        s.active_session()
            .and_then(|s| s.status.as_ref())
            .is_some_and(|s| s.job_kind.as_deref() == Some("repeat"))
    })
}
fn device_picker(store: Store) -> impl IntoElement {
    nsh_base::select()
        .items(move || {
            store.read(|s| {
                let mut ports = s
                    .ports
                    .iter()
                    .map(|p| SelectItem::new(p.path.clone(), p.display_name.clone()))
                    .collect::<Vec<_>>();
                let selected = s.preferences.get("port");
                if !selected.is_empty() && !s.ports.iter().any(|p| p.path == selected) {
                    ports.push(SelectItem::new(
                        selected.to_owned(),
                        format!("{selected}（已移除）"),
                    ));
                }
                if ports.is_empty() {
                    ports.push(SelectItem::new("", "未发现串口"));
                }
                ports
            })
        })
        .value(move || Some(SharedString::from(store.field("port"))))
        .on_change(move |v: &Option<SharedString>| {
            if let Some(v) = v {
                store.change(|s| s.set("port", v.to_string()))
            }
        })
        .disabled(move || store.locked("port") || store.read(|s| s.ports.is_empty()))
        .accessibility_label("串口设备")
        .popup_style(popup)
        .searchable(true)
        .render_item(|item: &nsh_base::SelectEntry| {
            view()
                .px_2()
                .py_1()
                .child(text(item.title.clone()).text_size(body_size))
        })
        .trigger(
            view()
                .opacity(move || {
                    if store.locked("port") || store.read(|s| s.ports.is_empty()) {
                        0.45_f32
                    } else {
                        1_f32
                    }
                })
                .h(px(20.))
                .px(px(10.))
                .rounded(px(5.))
                .bg(move || {
                    theme()
                        .color(
                            if store.locked("port") || store.read(|s| s.ports.is_empty()) {
                                "serial-control-disabled"
                            } else {
                                "serial-control"
                            },
                        )
                        .unwrap_or(theme().secondary)
                })
                .flex_row()
                .items_center()
                .gap_1()
                .child(
                    text(move || {
                        store.read(|s| {
                            s.ports
                                .iter()
                                .find(|p| p.path == s.preferences.get("port"))
                                .map(|p| p.display_name.clone())
                                .unwrap_or_else(|| {
                                    if s.preferences.get("port").is_empty() {
                                        "未发现串口".into()
                                    } else {
                                        format!("{}（已移除）", s.preferences.get("port"))
                                    }
                                })
                        })
                    })
                    .flex_1()
                    .text_size(body_size)
                    .text_color(move || {
                        if store.locked("port") || store.read(|s| s.ports.is_empty()) {
                            theme().foreground.opacity(0.3)
                        } else {
                            theme().foreground
                        }
                    }),
                )
                .child(icons::icon("select").size(px(12.))),
        )
}
pub(super) fn terminal(store: Store) -> View {
    let records = computed(move || {
        store.read(|s| {
            let Some(session) = s.active_session() else {
                return vec![];
            };
            let p = &s.preferences;
            let colors = crate::presentation::Colors::new(p.get("colors"), p.get("tx_color")).ok();
            crate::presentation::filtered(session.records(), p)
                .unwrap_or_default()
                .into_iter()
                .map(|f| {
                    let text = crate::presentation::display_text(f, p);
                    Record {
                        id: f.sequence,
                        time: record_time(&f.timestamp),
                        direction: if matches!(f.direction, Direction::Send) {
                            "TX"
                        } else {
                            "RX"
                        },
                        bytes: f.bytes.len(),
                        color: colors.as_ref().and_then(|c| c.for_frame(f, &text)),
                        json: serde_json::to_string_pretty(f).unwrap_or_default(),
                        text,
                    }
                })
                .collect::<Vec<_>>()
        })
    });
    let search_config = computed(move || {
        store.read(|s| {
            (
                s.preferences.get("filter").to_owned(),
                s.preferences.flag("search_regex"),
                s.preferences.flag("search_case"),
            )
        })
    });
    let search_cache = stored(None::<((String, bool, bool), Option<regex::Regex>)>);
    let matcher = move || {
        let key = search_config.get();
        search_cache.with(|cache| {
            if cache.as_ref().is_none_or(|cached| cached.0 != key) {
                let regex =
                    untracked(|| store.read(|s| crate::serial::search(&s.preferences))).ok();
                *cache = Some((key, regex));
            }
            cache.as_ref().and_then(|cached| cached.1.clone())
        })
    };
    let table = component_ref::<DataTable>();
    let table_width = signal(700_f32);
    let selected = signal(Vec::<u64>::new());
    let anchor = signal(None::<u64>);
    let widths = signal(BTreeMap::<String, f32>::new());
    let top = signal(None::<u64>);
    let session = stored(None::<u64>);
    let focused = signal(false);
    let focus_region = element_ref();
    focus_region.on_focus_in(move || focused.set(true));
    focus_region.on_focus_out(move || focused.set(false));
    effect(move || {
        let active = store.read(|s| s.active);
        if session.get() == active {
            return;
        }
        if let Some(previous) = session.replace(active) {
            store.terminal_state.with(|states| {
                states.insert(
                    previous,
                    TerminalState {
                        selected: selected.get_untracked(),
                        anchor: anchor.get_untracked(),
                        widths: widths.get_untracked(),
                        top_sequence: top.get_untracked(),
                    },
                );
            });
        }
        let saved = active
            .and_then(|id| store.terminal_state.with(|states| states.get(&id).cloned()))
            .unwrap_or_default();
        batch(|| {
            selected.set(saved.selected);
            anchor.set(saved.anchor);
            widths.set(saved.widths);
            top.set(saved.top_sequence);
        });
    });
    cleanup(move || {
        if let Some(id) = session.get() {
            store.terminal_state.try_with(|states| {
                states.insert(
                    id,
                    TerminalState {
                        selected: selected.get_untracked(),
                        anchor: anchor.get_untracked(),
                        widths: widths.get_untracked(),
                        top_sequence: top.get_untracked(),
                    },
                );
            });
        }
    });
    let copy_selected = move || {
        let selection = selected.get_untracked();
        if let Some(value) = records.with_untracked(|rows| selection_text(rows, &selection)) {
            set_clipboard(value);
        }
    };
    let select_all = move || {
        selected.set(records.with_untracked(|rows| rows.iter().map(|row| row.id).collect()))
    };
    super::commands::watch_edit_actions(
        store,
        move || focused.get() && !store.text_terminal.get(),
        move || {
            selected.with(|selection| {
                records.with(|rows| rows.iter().any(|row| selection.contains(&row.id)))
            })
        },
        copy_selected,
        move || records.with(|rows| !rows.is_empty()),
        select_all,
    );
    effect(move || {
        if focused.get() {
            on_key("mod+c", move |_| copy_selected());
            on_key("mod+a", move |_| select_all());
        }
    });
    let data = computed(move || {
        records.with(|r| {
            DataTableRows::new(r.len())
                .texts("time", r.iter().map(|r| r.time.clone()))
                .texts("direction", r.iter().map(|r| r.direction))
                .texts("size", r.iter().map(|r| r.bytes.to_string()))
                .texts("data", r.iter().map(|r| r.text.replace('\n', " ↵ ")))
        })
    });
    effect(move || {
        let ids = records.with(|rows| rows.iter().map(|r| r.id).collect::<Vec<_>>());
        if ids.is_empty() {
            return;
        }
        if let Some(api) = table.api::<DataTableApi>() {
            if store.flag("auto_scroll") {
                api.scroll_to_row((ids.len() - 1) as u32, Some(false));
            } else if let Some(id) = top.get_untracked()
                && let Some(index) = ids.iter().position(|row| *row == id)
            {
                api.scroll_to_row(index as u32, Some(false));
            }
        }
    });
    effect(move || {
        if let Some(jump) = store.jump.get() {
            let index = records.with_untracked(|r| r.iter().position(|r| r.id == jump));
            if let Some(index) = index {
                selected.set(vec![jump]);
                anchor.set(Some(jump));
                untracked(|| store.change(|s| s.flag("auto_scroll", false)));
                if let Some(api) = table.api::<DataTableApi>() {
                    api.scroll_to_row(index as u32, Some(false));
                }
            }
        }
    });
    view()
        .flex_1()
        .min_h_0()
        .child(
            view()
                .flex_1()
                .min_h_0()
                .relative()
                .element_ref(&focus_region)
                .on_layout(move |bounds: &Bounds<Points>| table_width.set(bounds.size.width.0))
                .child(
                    when(
                        move || !store.text_terminal.get(),
                        move || {
                            nsh_base::data_table()
                                .columns(move || {
                                    columns(
                                        store.flag("timestamps"),
                                        &widths.get(),
                                        table_width.get(),
                                    )
                                })
                                .data(data)
                                .row_height(px(32.))
                                .header_height(px(28.))
                                .column_selectable(false)
                                .stripe(false)
                                .movable_columns(false)
                                .resizable_columns(true)
                                .accessibility_label("串口数据终端")
                                .component_ref(&table)
                                .flex_1()
                                .classes(table_classes())
                                .select_on_modifier_press(false)
                                .selection(move || {
                                    selected
                                        .with(|ids| ids.last().copied())
                                        .and_then(|id| {
                                            records.with(|r| r.iter().position(|r| r.id == id))
                                        })
                                        .map(|index| {
                                            let mut s = DataTableSelection::default();
                                            s.row = Some(index as u32);
                                            s
                                        })
                                })
                                .row_classes(move || {
                                    records.with(|rows| {
                                        rows.iter()
                                            .map(|row| {
                                                if selected.with(|ids| ids.contains(&row.id)) {
                                                    "bg-primary/15".into()
                                                } else {
                                                    "".into()
                                                }
                                            })
                                            .collect::<Vec<_>>()
                                    })
                                })
                                .on_selection_change(move |value: &Option<DataTableSelection>| {
                                    if let Some(row) = value.and_then(|s| s.row).and_then(|index| {
                                        records
                                            .with_untracked(|r| r.get(index as usize).map(|r| r.id))
                                    }) {
                                        selected.set(vec![row]);
                                        anchor.set(Some(row));
                                    }
                                })
                                .on_row_press(move |p: &DataTablePress| {
                                    let ids = records.with_untracked(|rows| {
                                        rows.iter().map(|r| r.id).collect::<Vec<_>>()
                                    });
                                    let (next, next_anchor) = select_rows(
                                        &ids,
                                        &selected.get_untracked(),
                                        anchor.get_untracked(),
                                        p.row as usize,
                                        p.shift,
                                        p.platform || p.control,
                                    );
                                    selected.set(next);
                                    anchor.set(next_anchor);
                                    focused.set(true);
                                })
                                .on_column_resize(move |event: &DataTableEvent| {
                                    if let DataTableEvent::ColumnResize(key, size) = event {
                                        widths.update(|columns| {
                                            columns.insert(key.to_string(), size.0);
                                        });
                                    }
                                })
                                .on_visible_rows_change(move |range: &DataTableRange| {
                                    top.set(records.with_untracked(|r| {
                                        r.get(range.start as usize).map(|r| r.id)
                                    }))
                                })
                                .on_context_menu(move |_: &DataTableContextMenu| {
                                    use nsh_core::element::desktop::{MenuItem, popup_menu};
                                    drop(popup_menu(vec![
                                        MenuItem::new("复制")
                                            .disabled(selected.with_untracked(Vec::is_empty))
                                            .on_select(copy_selected),
                                        MenuItem::new("记录表…")
                                            .on_select(move || tools::open(store, "records")),
                                    ]));
                                })
                                .render_cell(move |cell: &DataTableCell| {
                                    let row = cell.row as usize;
                                    let data = cell.key.as_str() == "data";
                                    let secondary = matches!(cell.key.as_str(), "time" | "size");
                                    let value = cell.text.clone();
                                    let color = move || {
                                        if data {
                                            records
                                                .with(|rows| {
                                                    rows.get(row).and_then(|row| row.color)
                                                })
                                                .map(|value| nsh_core::color::rgb(value).into())
                                                .unwrap_or(theme().foreground)
                                        } else if secondary {
                                            theme().muted_foreground
                                        } else {
                                            theme().foreground
                                        }
                                    };
                                    if data {
                                        dynamic(move || {
                                            let key = search_config.get();
                                            let pieces = if key.0.is_empty() {
                                                vec![(value.to_string(), false)]
                                            } else {
                                                matcher()
                                                    .map(|query| {
                                                        highlight_pieces(value.as_str(), &query)
                                                    })
                                                    .unwrap_or_else(|| {
                                                        vec![(value.to_string(), false)]
                                                    })
                                            };
                                            text("")
                                                .w_full()
                                                .font_family(mono)
                                                .text_size(body_size)
                                                .text_color(color)
                                                .children(pieces.into_iter().map(
                                                    |(value, matched)| {
                                                        text(value).when(matched, |span| {
                                                            span.bg(move || {
                                                                theme()
                                                                    .color("search-highlight")
                                                                    .unwrap_or_else(|| {
                                                                        theme()
                                                                            .color("warning")
                                                                            .unwrap_or(
                                                                                theme().primary,
                                                                            )
                                                                    })
                                                                    .opacity(0.35)
                                                            })
                                                        })
                                                    },
                                                ))
                                        })
                                        .into_element()
                                    } else {
                                        let label = text(cell.text.clone())
                                            .w_full()
                                            .font_family(mono)
                                            .text_size(small_control_size)
                                            .when(cell.key.as_str() == "size", |text| {
                                                text.text_right()
                                            })
                                            .text_color(color);
                                        if cell.key.as_str() == "direction" {
                                            view()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(6.))
                                                .child(
                                                    icons::icon(if cell.text.as_str() == "RX" {
                                                        "arrow-down-left"
                                                    } else {
                                                        "arrow-up-right"
                                                    })
                                                    .size(px(11.))
                                                    .text_color(move || theme().primary),
                                                )
                                                .child(label)
                                                .into_element()
                                        } else {
                                            label.into_element()
                                        }
                                    }
                                })
                        },
                    )
                    .otherwise(move || {
                        readonly(move || {
                            Some(
                                records
                                    .with(|r| {
                                        r.iter()
                                            .map(|r| {
                                                format!(
                                                    "{} {} {}\n",
                                                    if store.flag("timestamps") {
                                                        &r.time
                                                    } else {
                                                        ""
                                                    },
                                                    r.direction,
                                                    r.text
                                                )
                                            })
                                            .collect::<String>()
                                    })
                                    .into(),
                            )
                        })
                    }),
                )
                .child(when(
                    move || records.with(Vec::is_empty),
                    move || {
                        view()
                            .absolute()
                            .inset_0()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(
                                icons::icon("serial-empty-cable")
                                    .size(px(44.))
                                    .translate_y(px(-16.))
                                    .text_color(move || {
                                        theme()
                                            .color("empty-symbol")
                                            .unwrap_or(theme().muted_foreground)
                                    })
                                    .into_element(),
                            )
                            .child(
                                text(move || {
                                    store.read(|s| {
                                        if s.active_session().is_some_and(|s| !s.frames.is_empty())
                                        {
                                            "没有匹配的数据"
                                        } else if s.serial_connected() {
                                            "等待设备数据"
                                        } else {
                                            "暂无收发记录"
                                        }
                                    })
                                })
                                .text_size(px(15.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(move || {
                                    theme()
                                        .color("empty-foreground")
                                        .unwrap_or(theme().muted_foreground)
                                }),
                            )
                            .child(
                                text(move || {
                                    store.read(|s| {
                                        if s.active_session().is_some_and(|s| !s.frames.is_empty())
                                        {
                                            "尝试其他搜索内容，或显示全部方向。"
                                        } else if s.serial_connected() {
                                            "设备数据会实时显示在这里。"
                                        } else {
                                            "在左侧选择串口并连接，即可开始收发。"
                                        }
                                    })
                                })
                                .text_size(px(15.))
                                .text_color(move || {
                                    theme()
                                        .color("empty-foreground")
                                        .unwrap_or(theme().muted_foreground)
                                }),
                            )
                    },
                )),
        )
        .child(
            view()
                .h(px(31.))
                .px_3()
                .py(px(5.))
                .flex_row()
                .items_center()
                .gap(px(10.))
                .border_t_1()
                .border_color(hairline)
                .child(view().w(px(82.)).child(popup_control(
                    store,
                    "display",
                    &["hex", "ascii", "gbk", "utf-8", "latin-1"],
                    true,
                )))
                .child(
                    text(move || format!("{} 条", records.with(Vec::len)))
                        .text_size(native_caption_size)
                        .text_color(move || theme().muted_foreground),
                )
                .child(view().flex_1())
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(4.))
                        .child(icon_toggle(
                            "arrow-down-to-line",
                            || "自动滚动",
                            move || store.flag("auto_scroll"),
                            || false,
                            move || {
                                store.change(|state| {
                                    state
                                        .flag("auto_scroll", !state.preferences.flag("auto_scroll"))
                                })
                            },
                            11.5,
                        ))
                        .child(icon_toggle(
                            "terminal-pause",
                            move || {
                                if store.read(|state| {
                                    state
                                        .active_session()
                                        .is_some_and(|session| session.frozen.is_some())
                                }) {
                                    "继续显示"
                                } else {
                                    "暂停显示"
                                }
                            },
                            move || {
                                store.read(|state| {
                                    state
                                        .active_session()
                                        .is_some_and(|session| session.frozen.is_some())
                                })
                            },
                            || false,
                            move || store.change(Workbench::pause),
                            7.5,
                        ))
                        .child(divider())
                        .child(native_borderless_menu_small_offset(
                            view()
                                .w(px(20.))
                                .h(px(20.))
                                .items_center()
                                .justify_center()
                                .label("显示选项")
                                .text_color(move || {
                                    theme()
                                        .color("terminal-menu-glyph")
                                        .unwrap_or(theme().foreground)
                                })
                                .child(icons::icon("settings")),
                            move || {
                                vec![
                                    native_toggle(store, "mixed", "文本与 HEX 混显"),
                                    native_toggle(store, "timestamps", "显示时间戳"),
                                    NativeMenuItem::new("着色规则…")
                                        .on_select(move || tools::open(store, "colors")),
                                    NativeMenuItem::separator(),
                                    NativeMenuItem::new("重置计数").on_select(move || {
                                        store.change(|state| state.serial_action("resetCounters"))
                                    }),
                                    native_toggle(store, "reset_on_clear", "清屏时重置计数"),
                                ]
                            },
                            -3.,
                        ))
                        .child(terminal_action(
                            "terminal-table",
                            "记录表",
                            16.,
                            || false,
                            move || tools::open(store, "records"),
                        ))
                        .child(native_borderless_menu_small_disabled(
                            view()
                                .w(px(19.))
                                .h(px(20.))
                                .items_center()
                                .justify_center()
                                .label("导出记录")
                                .text_color(move || {
                                    theme()
                                        .color(if serial_history_empty(store) {
                                            "terminal-menu-disabled"
                                        } else {
                                            "terminal-menu-glyph"
                                        })
                                        .unwrap_or(theme().muted_foreground)
                                })
                                .child(icons::icon("export")),
                            move || serial_history_empty(store),
                            move || {
                                ["json", "txt", "csv", "bin"]
                                    .into_iter()
                                    .map(|format| {
                                        NativeMenuItem::new(format.to_uppercase())
                                            .on_select(move || export_records(store, format))
                                    })
                                    .collect()
                            },
                        ))
                        .child(terminal_action(
                            "trash-portrait",
                            "清屏",
                            13.,
                            move || serial_history_empty(store),
                            move || store.change(Workbench::clear_terminal),
                        ))
                        .child(divider())
                        .child(icon_toggle(
                            "terminal-composer",
                            || "发送区",
                            move || store.composer.get(),
                            || false,
                            move || store.composer.update(|visible| *visible = !*visible),
                            15.5,
                        )),
                ),
        )
}
fn serial_history_empty(store: Store) -> bool {
    store.read(|state| {
        state
            .active_session()
            .is_none_or(|session| session.frames.is_empty())
    })
}
fn terminal_action(
    name: &'static str,
    label: &'static str,
    width: f32,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    let disabled = computed(disabled);
    nsh_base::button()
        .disabled(disabled)
        .on_press(press)
        .accessibility_label(label)
        .w(px(width))
        .h(px(20.))
        .p_0()
        .items_center()
        .justify_center()
        .hit_slop(nsh_core::geometry::Edges::all(px(5.)))
        .text_color(move || {
            theme()
                .color(if disabled.get() {
                    "terminal-button-disabled"
                } else {
                    "terminal-glyph"
                })
                .unwrap_or(theme().muted_foreground)
        })
        .focus_visible(|style| style.outline(px(2.)).outline_color(move || theme().ring))
        .child(icons::icon(name).size(px(if name == "terminal-table" { 15. } else { 14. })))
}

fn selection_text(rows: &[Record], selected: &[u64]) -> Option<String> {
    let rows = rows
        .iter()
        .filter(|row| selected.contains(&row.id))
        .collect::<Vec<_>>();
    (!rows.is_empty()).then(|| {
        rows.iter()
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    })
}

fn highlight_pieces(value: &str, query: &regex::Regex) -> Vec<(String, bool)> {
    // Native TerminalAppearance highlights the first match; Regex ranges are UTF-8 boundaries, Text spans convert them natively.
    let Some(found) = query.find(value).filter(|found| !found.is_empty()) else {
        return vec![(value.into(), false)];
    };
    let mut pieces = Vec::with_capacity(3);
    if found.start() > 0 {
        pieces.push((value[..found.start()].into(), false));
    }
    pieces.push((value[found.range()].into(), true));
    if found.end() < value.len() {
        pieces.push((value[found.end()..].into(), false));
    }
    pieces
}

fn columns(
    timestamps: bool,
    widths: &BTreeMap<String, f32>,
    available: f32,
) -> Vec<DataTableColumn> {
    [
        ("time", "时间", 116., 100.),
        ("direction", "方向", 76., 72.),
        ("size", "字节", 64., 52.),
        ("data", "数据", 620., 120.),
    ]
    .into_iter()
    .filter(|c| timestamps || c.0 != "time")
    .map(|(key, title, width, min)| {
        let mut c = DataTableColumn::default();
        c.key = key.into();
        c.title = Some(title.into());
        c.width = Some(px(if key == "data" && !widths.contains_key(key) {
            let leading = if timestamps {
                widths.get("time").copied().unwrap_or(116.)
            } else {
                0.
            } + widths.get("direction").copied().unwrap_or(76.)
                + widths.get("size").copied().unwrap_or(64.);
            (available - leading).max(min)
        } else {
            widths.get(key).copied().unwrap_or(width)
        }));
        c.min_width = Some(px(min));
        c
    })
    .collect()
}
fn table_classes() -> DataTableClasses {
    let mut c = DataTableClasses::default();
    c.header = Some("border-b border-border".into());
    c.header_cell = Some("px-3 border-r border-border text-xs".into());
    c.cell = Some("px-3 text-xs".into());
    c.row_selected = Some("bg-primary/15".into());
    c
}
fn export_records(store: Store, format: &'static str) {
    let bytes = store.read(|s| {
        let Some(session) = s.active_session() else {
            return Ok(vec![]);
        };
        let records = session.frames.iter().collect::<Vec<_>>();
        crate::presentation::records(&records, format, s.preferences.get("display"))
    });
    match bytes {
        Ok(bytes) => store.export(&format!("ChipDock-Serial.{format}"), bytes),
        Err(e) => store.report(e),
    }
}
fn divider() -> View {
    view().w(px(1.)).h(px(14.)).mx_1().bg(hairline)
}
fn checksum_menu(store: Store) -> impl IntoElement {
    let disabled = move || {
        store.read(|s| {
            s.installing
                || s.active_session()
                    .and_then(|s| s.status.as_ref())
                    .is_some_and(|s| s.job_kind.as_deref() == Some("repeat"))
        })
    };
    native_menu_small_offset_disabled(
        view()
            .w(px(86.5))
            .h(px(20.))
            .px_2()
            .flex_row()
            .items_center()
            .gap_1()
            .rounded(px(5.))
            .bg(move || theme().color("serial-control").unwrap_or(theme().secondary))
            .label("发送校验")
            .child(icons::icon("shield").size(px(13.)))
            .child(
                text(move || value_label("checksum", &store.field("checksum")))
                    .text_size(small_control_size),
            )
            .child(icons::icon("down").size(px(8.)).ml_1()),
        disabled,
        move || {
            let item = |value: &'static str, label: &'static str| {
                NativeMenuItem::new(label)
                    .checked(store.field("checksum") == value)
                    .on_select(move || store.change(|s| s.set("checksum", value.to_owned())))
            };
            vec![
                NativeMenuItem::new("校验算法").disabled(true),
                item("none", "无校验（默认）"),
                item("sum8", "校验和（SUM8）"),
                NativeMenuItem::separator(),
                NativeMenuItem::new("奇偶校验").disabled(true),
                item("oddParity", "奇校验"),
                item("evenParity", "偶校验"),
                NativeMenuItem::separator(),
                item("xor8", "异或校验（XOR8）"),
                item("crc16Modbus", "CRC16/MODBUS"),
                item("crc16Ccitt", "CRC16/CCITT"),
                item("crc32", "CRC32"),
                item("lrc", "LRC"),
            ]
        },
        3.,
    )
}

fn command_composer(store: Store) -> View {
    let input = element_ref();
    let focused = signal(false);
    input.on_focus_in(move || focused.set(true));
    input.on_focus_out(move || focused.set(false));
    effect(move || {
        if focused.get() {
            let keys = nsh_core::element::KeyOptions {
                when: nsh_core::keymap::When::Always,
                repeat: true,
                ..Default::default()
            };
            on_key_with("up", keys, move |_| recall_command(store, true));
            on_key_with("down", keys, move |_| recall_command(store, false));
        }
    });
    let preview = computed(move || store.read(|s| command_preview(&s.preferences)));
    let suggestions = computed(move || {
        store.read(|s| {
            serde_json::from_str::<Vec<chipdock_core::serial::program::Rule>>(
                s.preferences.get("protocol"),
            )
            .unwrap_or_default()
            .into_iter()
            .filter(|r| {
                !r.receive
                    && (s.preferences.get("command").is_empty()
                        || r.name.starts_with(s.preferences.get("command")))
            })
            .map(|r| r.name)
            .collect::<Vec<_>>()
        })
    });
    view()
        .flex_1()
        .min_h_0()
        .px(px(14.))
        .py(px(10.))
        .gap_2()
        .child(
            draft_input(
                "协议命令",
                move || store.field("command"),
                move |v| store.change(|s| s.set("command", v.into())),
            )
            .element_ref(&input)
            .on_submit(move |_: &str| {
                if store.read(super::commands::can_send) {
                    store.change(Workbench::send);
                }
            })
            .submit_behavior(SubmitBehavior::Submit)
            .placeholder("例如 motor-speed 45")
            .readonly(move || store.read(|s| s.installing))
            .font_family(mono)
            .text_size(body_size),
        )
        .child(when(
            move || suggestions.with(|v| !v.is_empty()),
            move || {
                scroll()
                    .horizontal()
                    .child(view().flex_row().gap_1().child(each(
                        suggestions,
                        |name: &String| name.clone(),
                        move |name, _| {
                            nsh_base::button()
                                .on_press(move || {
                                    store.change(|s| {
                                        s.set("command", format!("{} ", name.get_untracked()))
                                    })
                                })
                                .h(px(20.))
                                .px_2()
                                .rounded(px(5.))
                                .hover(|s| s.bg(move || theme().accent))
                                .child(text(move || name.get()).text_size(caption_size))
                        },
                    )))
            },
        ))
        .child(when(
            move || preview.with(|p| !p.0.is_empty()),
            move || {
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(8.5))
                    .child(
                        when(
                            move || preview.with(|p| p.1),
                            || {
                                icons::icon("warning-fill")
                                    .size(px(12.))
                                    .text_color(move || theme().destructive)
                            },
                        )
                        .otherwise(|| icons::icon("up").size(px(12.))),
                    )
                    .child(
                        text(move || preview.with(|p| p.0.clone()))
                            .text_size(native_caption_size)
                            .font_family(mono)
                            .text_color(move || {
                                if preview.with(|p| p.1) {
                                    theme().destructive
                                } else {
                                    theme().muted_foreground
                                }
                            }),
                    )
            },
        ))
        .child(when(
            move || {
                store.read(|s| {
                    s.active_session()
                        .and_then(|s| s.status.as_ref())
                        .and_then(|s| s.feedback.as_ref())
                        .is_some()
                })
            },
            move || {
                caption(move || {
                    store.read(|s| {
                        s.active_session()
                            .and_then(|s| s.status.as_ref())
                            .and_then(|s| s.feedback.clone())
                            .unwrap_or_default()
                    })
                })
            },
        ))
}

fn recall_command(store: Store, previous: bool) {
    store.change(|s| {
        let history = &s.preferences.command_history;
        if history.is_empty() {
            return;
        }
        let at = history
            .iter()
            .position(|v| v == s.preferences.get("command"));
        let next = match (at, previous) {
            (None, true) => Some(0),
            (None, false) => None,
            (Some(at), true) => Some((at + 1).min(history.len() - 1)),
            (Some(at), false) => at.checked_sub(1),
        };
        let value = next
            .and_then(|at| history.get(at))
            .cloned()
            .unwrap_or_default();
        s.set("command", value);
    });
}

pub(super) fn command_preview(preferences: &crate::model::Preferences) -> (String, bool) {
    let input = preferences.get("command");
    let rules = match serde_json::from_str::<serde_json::Value>(preferences.get("protocol")) {
        Ok(rules) => rules,
        Err(error) => return (error.to_string(), true),
    };
    let request = serde_json::json!({"operation":"encode","rules":rules,"input":input});
    match chipdock_core::serial::program::execute(&request.to_string()) {
        Ok(result) => match serde_json::from_str::<serde_json::Value>(&result) {
            Ok(value) => (value["hex"].as_str().unwrap_or("").into(), false),
            Err(error) => (error.to_string(), true),
        },
        Err(error) => (error.to_string(), true),
    }
}

fn record_time(timestamp: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(timestamp).map_or_else(
        |_| timestamp.to_owned(),
        |date| {
            date.with_timezone(&chrono::Local)
                .format("%H:%M:%S%.3f")
                .to_string()
        },
    )
}

fn connection_uptime(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    format!(
        "{}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut output = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(digit);
    }
    output
}

fn config_summary(config: &chipdock_core::serial::types::SerialConfig) -> String {
    use chipdock_core::serial::types::Parity;
    format!(
        "{} · {}{}{}",
        config.baud_rate,
        config.data_bits,
        match config.parity {
            Parity::Even => "E",
            Parity::Odd => "O",
            _ => "N",
        },
        config.stop_bits
    )
}

pub(super) fn status_bar(store: Store) -> View {
    let separator = || view().w(px(1.)).h(px(12.)).bg(hairline);
    let status_label = move || {
        store.read(|state| {
            let reconnecting = state
                .active_session()
                .and_then(|session| session.status.as_ref())
                .is_some_and(|status| status.reconnecting);
            if state.serial_busy() {
                "正在处理…"
            } else if reconnecting {
                "正在重连…"
            } else if state.serial_connected() {
                "已连接"
            } else {
                "未连接"
            }
        })
    };
    view()
        .h(px(27.))
        .px(px(14.))
        .flex_row()
        .items_center()
        .gap(px(10.))
        .border_t_1()
        .border_color(hairline)
        .text_size(native_caption_size)
        .text_color(move || theme().muted_foreground)
        .role(AccessibilityRole::Text)
        .accessibility_value(move || {
            let label = status_label();
            Some(nsh_core::AccessibilityValue {
                text: Some(
                    store
                        .read(|state| {
                            let status = state
                                .active_session()
                                .and_then(|session| session.status.as_ref());
                            format!(
                                "{}、{} B、{} B",
                                label,
                                grouped(status.map_or(0, |status| status.rx_bytes)),
                                grouped(status.map_or(0, |status| status.tx_bytes))
                            )
                        })
                        .into(),
                ),
                ..Default::default()
            })
        })
        .accessibility_elements_hidden(true)
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .child(view().size(px(7.)).rounded_full().bg(move || {
                    store.read(|state| {
                        let status = state
                            .active_session()
                            .and_then(|session| session.status.as_ref());
                        if state.serial_connected() {
                            theme().color("success").unwrap_or(theme().primary)
                        } else if status.is_some_and(|status| status.reconnecting) {
                            theme().color("warning").unwrap_or(theme().primary)
                        } else {
                            theme().muted_foreground.opacity(0.4)
                        }
                    })
                }))
                .child(text(status_label).text_color(move || theme().foreground)),
        )
        .child(when(
            move || store.read(|state| state.serial_connected()),
            move || {
                view()
                    .min_w_0()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(separator())
                    .child(text(move || {
                        store.read(|state| {
                            state
                                .active_session()
                                .and_then(|session| session.status.as_ref())
                                .and_then(|status| status.path.clone())
                                .unwrap_or_default()
                        })
                    }))
                    .child(separator())
                    .child(text(move || {
                        store.read(|state| {
                            state
                                .active_session()
                                .and_then(|session| session.status.as_ref())
                                .and_then(|status| status.config.as_ref())
                                .map(config_summary)
                                .or_else(|| {
                                    crate::backend::serial_config(&state.preferences)
                                        .ok()
                                        .map(|config| config_summary(&config))
                                })
                                .unwrap_or_default()
                        })
                    }))
            },
        ))
        .child(view().flex_1().min_w(px(8.)))
        .child(when(
            move || {
                store.read(|state| {
                    state
                        .active_session()
                        .and_then(|session| session.status.as_ref())
                        .is_some_and(|status| status.logging || status.capturing)
                })
            },
            move || {
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap(px(4.))
                            .text_color(move || theme().destructive)
                            .child(
                                view()
                                    .size(px(8.))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(move || theme().destructive),
                            )
                            .child(text("采集中")),
                    )
                    .child(separator())
            },
        ))
        .child(when(
            move || store.read(|state| state.serial_connected()),
            move || {
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(view().tooltip(text("连接时长")).child(text(move || {
                        store.read(|state| {
                            state
                                .active_session()
                                .and_then(|session| session.status.as_ref())
                                .map_or_else(String::new, |status| {
                                    connection_uptime(status.uptime_ms)
                                })
                        })
                    })))
                    .child(separator())
                    .child(text(move || {
                        store.read(|state| {
                            state
                                .active_session()
                                .and_then(|session| session.status.as_ref())
                                .map_or_else(String::new, |status| {
                                    format!(
                                        "{} / {} B/s",
                                        status.rx_per_second, status.tx_per_second
                                    )
                                })
                        })
                    }))
                    .child(separator())
            },
        ))
        .children([(true, "arrow-down"), (false, "arrow-up")].into_iter().map(
            move |(receive, icon)| {
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(3.))
                    .child(icons::icon(icon).size(px(11.)))
                    .child(text(move || {
                        store.read(|state| {
                            format!(
                                "{} B",
                                grouped(
                                    state
                                        .active_session()
                                        .and_then(|session| session.status.as_ref())
                                        .map_or(0, |status| if receive {
                                            status.rx_bytes
                                        } else {
                                            status.tx_bytes
                                        })
                                )
                            )
                        })
                    }))
            },
        ))
}
pub(super) fn error_banner(store: Store) -> impl IntoElement {
    let dismissed = signal(String::new());
    let error = computed(move || {
        store.read(|s| {
            s.active_session()
                .and_then(|s| s.status.as_ref())
                .and_then(|s| {
                    s.last_error
                        .as_ref()
                        .or(s.job_error.as_ref())
                        .or(s.capture_error.as_ref())
                })
                .cloned()
                .or_else(|| {
                    crate::serial::search(&s.preferences)
                        .err()
                        .map(|error| format!("搜索表达式无效：{error}"))
                })
                .or_else(|| {
                    crate::presentation::Colors::new(
                        s.preferences.get("colors"),
                        s.preferences.get("tx_color"),
                    )
                    .err()
                    .map(|error| format!("着色规则无效：{error}"))
                })
                .unwrap_or_default()
        })
    });
    when(
        move || !error.get().is_empty() && error.get() != dismissed.get(),
        move || {
            view()
                .p_3()
                .flex_row()
                .items_start()
                .gap(px(10.))
                .bg(move || {
                    theme()
                        .color("warning")
                        .unwrap_or(theme().primary)
                        .opacity(0.08)
                })
                .child(icons::icon("warning"))
                .child(text(move || error.get()).flex_1().text_size(caption_size))
                .child(icon_button(
                    "close",
                    "关闭错误提示",
                    || false,
                    || false,
                    move || dismissed.set(error.get_untracked()),
                ))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copy_uses_visible_sequence_selection_and_keeps_empty_selection_untouched() {
        let row = Record {
            id: 9,
            time: String::new(),
            direction: "RX",
            bytes: 0,
            text: String::new(),
            color: None,
            json: String::new(),
        };
        assert_eq!(
            super::selection_text(std::slice::from_ref(&row), &[99]),
            None
        );
        assert_eq!(super::selection_text(std::slice::from_ref(&row), &[]), None);
        assert_eq!(
            super::selection_text(std::slice::from_ref(&row), &[9]),
            Some(String::new())
        );
    }

    #[test]
    fn native_status_and_time_formats() {
        assert_eq!(super::grouped(1_234_567), "1,234,567");
        assert_eq!(super::connection_uptime(3_661_999), "1:01:01");
        assert_eq!(super::config_summary(&Default::default()), "115200 · 8N1");
        assert_eq!(super::record_time("invalid"), "invalid");
        let timestamp = chrono::DateTime::parse_from_rfc3339("2026-11-14T12:34:56.123Z").unwrap();
        assert_eq!(
            super::record_time("2026-11-14T12:34:56.123Z"),
            timestamp
                .with_timezone(&chrono::Local)
                .format("%H:%M:%S%.3f")
                .to_string()
        );
    }

    #[test]
    fn search_navigation_uses_original_sequences_and_wraps() {
        let ids = [12, 31, 98];
        assert_eq!(next_sequence(&ids, None, false), Some(98));
        assert_eq!(next_sequence(&ids, Some(31), true), Some(98));
        assert_eq!(next_sequence(&ids, Some(98), true), Some(12));
        assert_eq!(next_sequence(&ids, Some(7), true), Some(12));
    }
    #[test]
    fn search_highlight_preserves_unicode_and_native_first_match() {
        let query = regex::Regex::new("42").unwrap();
        assert_eq!(
            highlight_pieces("温度42°C 42", &query),
            vec![
                ("温度".into(), false),
                ("42".into(), true),
                ("°C 42".into(), false)
            ]
        );
        assert_eq!(
            highlight_pieces("温度", &regex::Regex::new("^").unwrap()),
            vec![("温度".into(), false)]
        );
    }
    #[test]
    fn modifier_selection_preserves_domain_ids() {
        let ids = [12, 31, 98, 105];
        let (selected, anchor) = select_rows(&ids, &[12], Some(12), 2, true, false);
        assert_eq!(selected, [12, 31, 98]);
        let (selected, _) = select_rows(&ids, &selected, anchor, 1, false, true);
        assert_eq!(selected, [12, 98]);
    }
}
