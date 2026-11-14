//! Native application menus. Availability and dispatch share the same live guards.
use super::*;
use nsh_core::desktop::MenuRole;
use nsh_core::element::desktop::{MenuItem as M, current_window, set_application_menu};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy)]
pub(super) struct Context {
    kind: &'static str,
    search: Option<ElementRef>,
    registration: u64,
}

impl Context {
    pub(super) fn main() -> Self {
        Self {
            kind: "main",
            search: None,
            registration: 0,
        }
    }
}

impl PartialEq for Context {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.registration == other.registration
    }
}

#[derive(Clone)]
pub(super) struct EditActions {
    registration: u64,
    window_kind: &'static str,
    can_copy: bool,
    can_select_all: bool,
    copy: Rc<dyn Fn()>,
    select_all: Rc<dyn Fn()>,
}

impl PartialEq for EditActions {
    fn eq(&self, other: &Self) -> bool {
        self.registration == other.registration
            && self.can_copy == other.can_copy
            && self.can_select_all == other.can_select_all
    }
}

/// Registers a focused collection's existing selection commands, without
/// copying its selection or intercepting text editing in another control.
pub(super) fn watch_edit_actions(
    store: Store,
    focused: impl Fn() -> bool + 'static,
    can_copy: impl Fn() -> bool + 'static,
    copy: impl Fn() + 'static,
    can_select_all: impl Fn() -> bool + 'static,
    select_all: impl Fn() + 'static,
) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let registration = NEXT.fetch_add(1, Ordering::Relaxed);
    let window = current_window();
    let window_kind = match window.id() {
        "main" => "main",
        "logs" => "logs",
        "memory" => "memory",
        "settings" => "settings",
        _ => "plot",
    };
    let copy: Rc<dyn Fn()> = Rc::new(copy);
    let select_all: Rc<dyn Fn()> = Rc::new(select_all);
    effect(move || {
        if window.state().is_some_and(|window| window.focused) && focused() {
            store.edit_actions.set(Some(EditActions {
                registration,
                window_kind,
                can_copy: can_copy(),
                can_select_all: can_select_all(),
                copy: copy.clone(),
                select_all: select_all.clone(),
            }));
        } else if store
            .edit_actions
            .get_untracked()
            .is_some_and(|actions| actions.registration == registration)
        {
            store.edit_actions.set(None);
        }
    });
    cleanup(move || {
        if store
            .edit_actions
            .try_get()
            .flatten()
            .is_some_and(|actions| actions.registration == registration)
        {
            store.edit_actions.set(None);
        }
    });
}

fn edit_actions(store: Store, context: Context) -> Option<EditActions> {
    store
        .edit_actions
        .get_untracked()
        .filter(|actions| actions.window_kind == context.kind)
}

fn collection_edit_item(store: Store, context: Context, copy: bool) -> M {
    let enabled = edit_actions(store, context).is_some_and(|actions| {
        if copy {
            actions.can_copy
        } else {
            actions.can_select_all
        }
    });
    M::new(if copy { "拷贝" } else { "全选" })
        .shortcut(if copy { "mod+c" } else { "mod+a" })
        .shortcut_when(nsh_core::keymap::When::Auto)
        .disabled(!enabled)
        .on_select(move || {
            let context = store.command_context.get_untracked();
            if let Some(actions) = edit_actions(store, context) {
                if copy && actions.can_copy {
                    (actions.copy)();
                } else if !copy && actions.can_select_all {
                    (actions.select_all)();
                }
            }
        })
}

/// A window owns its search ref; command handlers keep that ref while it is focused.
pub(super) fn watch_window(store: Store, kind: &'static str, search: Option<ElementRef>) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let context = Context {
        kind,
        search,
        registration: NEXT.fetch_add(1, Ordering::Relaxed),
    };
    let window = current_window();
    effect(move || {
        if window.state().is_some_and(|state| state.focused) {
            store.command_context.set(context);
        }
    });
}

/// The native Find command focuses and selects the current query. Pointer
/// focus and the clear button keep their existing caret behavior.
pub(super) fn focus_search(search: ElementRef, value: &str) {
    search.focus();
    search.set_selection(
        0,
        value.encode_utf16().count().min(u32::MAX as usize) as u32,
    );
}

pub(super) fn can_send(state: &Workbench) -> bool {
    if !state.serial_connected()
        || state.serial_busy()
        || state
            .active_session()
            .and_then(|session| session.status.as_ref())
            .is_some_and(|status| status.job_kind.is_some())
    {
        return false;
    }
    if state.preferences.flag("command_mode") {
        let (preview, invalid) = serial::command_preview(&state.preferences);
        !state.preferences.get("command").is_empty() && !invalid && !preview.is_empty()
    } else {
        !state.preferences.get("send").is_empty()
    }
}

fn available(store: Store) -> bool {
    !store.modal.get() && !store.file_dialog.get() && !store.read(|s| s.installing)
}
pub(super) fn probe_idle(store: Store) -> bool {
    available(store) && !store.read(|s| s.lane_busy(Lane::Probe))
}
fn target_sheet(store: Store) -> bool {
    !store.file_dialog.get()
        && store.target_setup.get()
        && !store.target_chips.get()
        && store.target_draft.with(|draft| draft.is_some())
        && !store.read(|s| s.lane_busy(Lane::Probe))
}
fn can_connect_target(store: Store) -> bool {
    if target_sheet(store) {
        target::can_connect_from_menu(store)
    } else {
        probe_idle(store)
            && store.read(|state| {
                state.preferences.target().is_ok()
                    && (!state.probes_scanned || !state.probes.is_empty())
            })
    }
}
fn can_read(store: Store) -> bool {
    probe_idle(store) && store.read(can_read_memory)
}
pub(super) fn can_read_memory(state: &Workbench) -> bool {
    if !state.target_connected() || state.lane_busy(Lane::Probe) {
        return false;
    }
    match (
        state.preferences.number("address"),
        state.preferences.number("length"),
    ) {
        (Ok(address), Ok(length)) => valid_memory_range(address, length),
        _ => false,
    }
}
fn valid_memory_range(address: u64, length: u64) -> bool {
    (1..=4096).contains(&length) && address.checked_add(length).is_some()
}
fn can_flash(store: Store) -> bool {
    probe_idle(store)
        && store.read(|s| {
            s.target_connected()
                && s.preview.is_some()
                && !s.preferences.get("firmware").is_empty()
                && s.preferences.firmware().is_ok()
        })
}
fn can_erase(store: Store) -> bool {
    probe_idle(store) && store.read(Workbench::target_connected)
}
fn has_logs(store: Store) -> bool {
    store.read(|s| !s.logs.is_empty())
}
fn repeating(store: Store) -> bool {
    store.read(|s| {
        s.active_session()
            .and_then(|session| session.status.as_ref())
            .is_some_and(|status| status.job_kind.as_deref() == Some("repeat"))
    })
}

fn item(
    store: Store,
    label: impl Into<String>,
    allowed: impl Fn(Store) -> bool + 'static,
    action: impl Fn(Store) + 'static,
) -> M {
    let disabled = !allowed(store);
    M::new(label).disabled(disabled).on_select(move || {
        if allowed(store) {
            action(store)
        }
    })
}
fn toggle_flag(store: Store, key: &'static str) {
    store.change(|s| s.flag(key, !s.preferences.flag(key)));
}
fn role(role: MenuRole) -> M {
    let label = match role {
        MenuRole::About => "关于 ChipDock",
        MenuRole::Hide => "隐藏 ChipDock",
        MenuRole::HideOthers => "隐藏其他",
        MenuRole::ShowAll => "全部显示",
        MenuRole::Quit => "退出 ChipDock",
        MenuRole::Undo => "撤销",
        MenuRole::Redo => "重做",
        MenuRole::Cut => "剪切",
        MenuRole::Copy => "拷贝",
        MenuRole::Paste => "粘贴",
        MenuRole::SelectAll => "全选",
        MenuRole::Minimize => "最小化",
        MenuRole::Zoom => "缩放",
        MenuRole::ToggleFullScreen => "进入全屏幕",
        MenuRole::Close => "关闭",
    };
    M::role(role).label(label)
}
fn icon(item: M, name: &str) -> M {
    if cfg!(target_os = "macos") {
        item
    } else {
        item.icon(icons::source(name))
    }
}
fn panel_open(store: Store, id: &str) -> bool {
    store.revision.get();
    store.windows.with(|windows| {
        windows
            .get(id)
            .is_some_and(|window| window.state().is_some())
    })
}
pub(super) fn toggle_panel(store: Store, id: &str, panel: auxiliary::Panel) {
    let window = store.windows.with(|windows| windows.get(id).cloned());
    if let Some(window) = window.filter(|window| window.state().is_some()) {
        window.close();
    } else {
        auxiliary::open(store, panel)
    }
}
fn export_logs(store: Store) {
    store.export_as(
        "ChipDock.log",
        store
            .read(|s| s.logs.iter().cloned().collect::<Vec<_>>().join("\n"))
            .into_bytes(),
        file_dialogs::ExportKind::DocumentText,
    );
}
fn export_memory(store: Store) {
    match store.read(|s| {
        s.memory
            .as_ref()
            .map(|memory| (memory.address, crate::model::decode_hex(&memory.data_hex)))
    }) {
        Some((address, Ok(bytes))) => store.export_as(
            &format!("memory-0x{address:08X}.bin"),
            bytes,
            file_dialogs::ExportKind::DocumentData,
        ),
        Some((_, Err(error))) => store.report(error),
        None => {}
    }
}

/// Only state visible in the menus belongs here. Terminal frames, uptime and
/// progress ticks must not replace an open native menu or its live handlers.
#[derive(PartialEq, Eq)]
struct MenuKey {
    serial: bool,
    context: (&'static str, u64),
    flags: Vec<bool>,
    direction: String,
    recent: Vec<(String, bool)>,
    profiles: Vec<String>,
    edit: Option<(u64, bool, bool)>,
}

fn menu_key(store: Store, mode: Signal<&'static str>, context: Context) -> MenuKey {
    let serial = mode.get() == "serial" && context.kind != "memory";
    let (recent, profiles, state_flags, direction) = store.read(|state| {
        let preferences = &state.preferences;
        (
            preferences
                .recent
                .iter()
                .map(|path| (path.clone(), std::path::Path::new(path).is_file()))
                .collect(),
            preferences.profiles.keys().cloned().collect(),
            vec![
                state.installing,
                state.serial_busy(),
                state.active.is_some(),
                serial_status_active(state),
                can_send(state),
                preferences.target().is_ok(),
                preferences.get("firmware").is_empty(),
                preferences.get("port").is_empty(),
                crate::backend::serial_config(preferences).is_ok(),
                preferences
                    .number("interval")
                    .is_ok_and(|value| (10..=3_600_000).contains(&value)),
                preferences.flag("command_mode"),
                preferences.flag("dry_run"),
                preferences.flag("auto_scroll"),
                preferences.flag("errors_only"),
                preferences.flag("follow_logs"),
                state.memory.is_some(),
                state.target.is_some(),
                state.can_quit(),
                state.lane_busy(Lane::Discovery),
                state.lane_busy(Lane::Update),
                state.probes_scanned,
                state.probes.is_empty(),
                state
                    .active_session()
                    .is_some_and(|session| session.records().next().is_some()),
            ],
            preferences.get("direction").to_owned(),
        )
    });
    let mut flags = vec![
        store.modal.get(),
        store.file_dialog.get(),
        store.target_setup.get(),
        store.target_chips.get(),
        can_connect_target(store),
        available(store),
        probe_idle(store),
        can_read(store),
        can_flash(store),
        can_erase(store),
        has_logs(store),
        repeating(store),
        store.sidebar.get(),
        store.composer.get(),
        panel_open(store, "logs"),
        panel_open(store, "memory"),
    ];
    flags.extend(state_flags);
    MenuKey {
        serial,
        context: (context.kind, context.registration),
        flags,
        direction,
        recent,
        profiles,
        edit: store
            .edit_actions
            .get()
            .filter(|actions| actions.window_kind == context.kind)
            .map(|actions| {
                (
                    actions.registration,
                    actions.can_copy,
                    actions.can_select_all,
                )
            }),
    }
}

pub(super) fn install(store: Store, mode: Signal<&'static str>) {
    #[cfg(target_os = "macos")]
    super::commands_macos::install();
    #[cfg(target_os = "macos")]
    {
        let modal = computed(move || {
            store.modal.get() || store.file_dialog.get() || store.read(|state| state.installing)
        });
        effect(move || super::commands_macos::set_main_modal(modal.get()));
    }
    watch_window(store, "main", Some(store.search));
    follow_menu_state(
        move || menu_key(store, mode, store.command_context.get()),
        move || {
            let context = store.command_context.get_untracked();
            let serial_active = mode.get() == "serial" && context.kind != "memory";
            let file = vec![
                icon(
                    item(store, "打开固件…", probe_idle, move |store| {
                        mode.set("flash");
                        store.pick("firmware")
                    })
                    .shortcut("mod+o"),
                    "folder",
                ),
                recent_menu(store, mode),
                M::separator(),
                item(
                    store,
                    "关闭固件",
                    |store| probe_idle(store) && !store.field("firmware").is_empty(),
                    |store| store.change(|s| s.set("firmware", String::new())),
                ),
                item(
                    store,
                    if cfg!(target_os = "macos") {
                        "在访达中显示固件"
                    } else {
                        "在文件管理器中显示固件"
                    },
                    |store| !store.field("firmware").is_empty(),
                    |store| {
                        let path = store.field("firmware");
                        store.spawn(async move {
                            if let Err(error) = reveal_path(path).await {
                                store.report(error)
                            }
                        });
                    },
                ),
                M::separator(),
                role(MenuRole::Close),
                M::separator(),
                item(
                    store,
                    "导出日志…",
                    |store| {
                        has_logs(store)
                            && (available(store)
                                || store.command_context.get_untracked().kind == "logs")
                    },
                    export_logs,
                ),
                item(
                    store,
                    "导出内存…",
                    |store| {
                        store.read(|s| s.memory.is_some() && !s.lane_busy(Lane::Probe))
                            && (available(store)
                                || store.command_context.get_untracked().kind == "memory")
                    },
                    export_memory,
                ),
            ];
            // Keep the GPUI preference shortcut while matching macOS's File menu.
            let file = {
                let mut file = file;
                file.push(
                    item(store, "保存设置", available, |store| store.save()).shortcut("mod+s"),
                );
                file
            };
            let edit = vec![
                role(MenuRole::Undo),
                role(MenuRole::Redo),
                M::separator(),
                role(MenuRole::Cut),
                if edit_actions(store, context).is_some() {
                    collection_edit_item(store, context, true)
                } else {
                    role(MenuRole::Copy)
                },
                role(MenuRole::Paste),
                if edit_actions(store, context).is_some() {
                    collection_edit_item(store, context, false)
                } else {
                    role(MenuRole::SelectAll)
                },
                M::separator(),
                item(
                    store,
                    "查找…",
                    move |store| {
                        let context = store.command_context.get_untracked();
                        context.search.is_some()
                            && (context.kind == "logs"
                                || (context.kind == "main" && mode.get_untracked() == "serial"))
                            && available(store)
                    },
                    |store| {
                        let context = store.command_context.get_untracked();
                        if let Some(search) = context.search {
                            let value = store.field(if context.kind == "logs" {
                                "log_search"
                            } else {
                                "filter"
                            });
                            focus_search(search, &value);
                        }
                    },
                )
                .shortcut("mod+f"),
            ];
            let target = if serial_active {
                serial_targets(store)
            } else {
                flash_targets(store, mode)
            };
            let actions = match context.kind {
                "logs" => log_actions(store),
                "memory" => vec![
                    item(store, "读取内存", can_read, |store| {
                        store.command(|s| Command::Memory(s.preferences.clone()))
                    })
                    .shortcut("mod+shift+r"),
                    item(store, "擦除整个芯片…", can_erase, auxiliary::erase_memory),
                ],
                _ if serial_active => serial_actions(store),
                _ => vec![
                    icon(
                        item(
                            store,
                            if store.flag("dry_run") {
                                "开始预检查"
                            } else {
                                "烧录固件"
                            },
                            can_flash,
                            |store| flash::execute(store, false),
                        )
                        .shortcut("mod+enter"),
                        "arrow-down",
                    ),
                    item(store, "读取内存", can_read, |store| {
                        auxiliary::open(store, auxiliary::Panel::Memory);
                        store.command(|s| Command::Memory(s.preferences.clone()));
                    })
                    .shortcut("mod+shift+r"),
                    M::separator(),
                    item(store, "擦除整个芯片…", can_erase, flash::confirm_erase),
                ],
            };
            let mut view = vec![
                icon(
                    item(store, "固件烧录", available, move |_| mode.set("flash"))
                        .shortcut("mod+1"),
                    "memory",
                ),
                icon(
                    item(store, "串口连接", available, move |_| {
                        mode.set("serial")
                    })
                    .shortcut("mod+2"),
                    "terminal",
                ),
            ];
            if serial_active {
                view.extend([
                    item(store, "连接区", available, |store| {
                        store.sidebar.update(|open| *open = !*open)
                    })
                    .checked(store.sidebar.get()),
                    item(store, "发送区", available, |store| {
                        store.composer.update(|open| *open = !*open)
                    })
                    .checked(store.composer.get()),
                    M::submenu(
                        "数据方向",
                        [("all", "全部"), ("receive", "接收"), ("send", "发送")]
                            .into_iter()
                            .map(|(value, label)| {
                                item(store, label, available, move |store| {
                                    store.change(|s| s.set("direction", value.into()))
                                })
                                .checked(store.field("direction") == value)
                            }),
                    )
                    .disabled(!available(store)),
                ]);
            }
            view.extend([
                M::separator(),
                icon(
                    item(
                        store,
                        if panel_open(store, "logs") {
                            "隐藏日志"
                        } else {
                            "显示日志"
                        },
                        |_| true,
                        |store| toggle_panel(store, "logs", auxiliary::Panel::Logs),
                    )
                    .shortcut("mod+j"),
                    "logs",
                ),
                icon(
                    item(
                        store,
                        if panel_open(store, "memory") {
                            "隐藏内存"
                        } else {
                            "显示内存"
                        },
                        |_| true,
                        |store| toggle_panel(store, "memory", auxiliary::Panel::Memory),
                    )
                    .shortcut("mod+alt+m"),
                    "memory",
                ),
                M::separator(),
                role(MenuRole::ToggleFullScreen).disabled(!available(store)),
            ]);
            set_application_menu(Some(vec![
                application_menu(store),
                M::submenu("文件", file),
                M::submenu("编辑", edit),
                M::submenu("显示", view),
                M::submenu("目标", target),
                M::submenu("操作", actions),
                window_menu(store),
                M::submenu(
                    "帮助",
                    [
                        (
                            "ChipDock 源代码",
                            "https://github.com/tangxiangong/chipdock",
                        ),
                        ("probe-rs 文档", "https://probe.rs/docs/"),
                    ]
                    .into_iter()
                    .map(|(label, url)| {
                        M::new(label).on_select(move || {
                            store.spawn(async move {
                                let _ = open_url(url).await;
                            });
                        })
                    }),
                ),
            ]));
        },
    );
}

// NSH clears a menu effect's native handlers during cleanup. Dedupe its
// dependencies first so unrelated polling cannot rerun that effect and leave
// the still-visible native menu with an empty handler table.
fn follow_menu_state<T: PartialEq + 'static>(
    snapshot: impl Fn() -> T + 'static,
    install: impl Fn() + 'static,
) {
    let state = computed(snapshot);
    effect(move || {
        state.with(|_| ());
        untracked(&install);
    });
}

fn window_menu(store: Store) -> M {
    let items = vec![
        role(MenuRole::Minimize),
        role(MenuRole::Zoom),
        M::separator(),
        M::new("ChipDock").on_select(|| nsh_core::element::desktop::main_window().focus()),
        M::new("活动日志").on_select(move || auxiliary::open(store, auxiliary::Panel::Logs)),
        M::new("目标内存").on_select(move || auxiliary::open(store, auxiliary::Panel::Memory)),
    ];
    #[cfg(target_os = "macos")]
    let items = {
        let mut items = items;
        items.extend([M::separator(), M::new("前置全部窗口")]);
        items
    };
    M::submenu("窗口", items)
}

fn application_menu(store: Store) -> M {
    let settings = item(store, "设置…", available, |store| {
        auxiliary::open(store, auxiliary::Panel::Settings)
    })
    .shortcut("mod+,");
    #[cfg(not(target_os = "macos"))]
    let settings = settings.icon(icons::source("gear"));
    let mut items = vec![
        role(MenuRole::About),
        item(
            store,
            "检查更新…",
            |store| available(store) && !store.read(|s| s.lane_busy(Lane::Update)),
            auxiliary::check_update,
        ),
        M::separator(),
        settings,
        M::separator(),
    ];
    #[cfg(target_os = "macos")]
    items.extend([
        M::submenu("服务", []),
        M::separator(),
        role(MenuRole::Hide),
        role(MenuRole::HideOthers),
        role(MenuRole::ShowAll),
        M::separator(),
    ]);
    items.push(
        item(
            store,
            "退出 ChipDock",
            |store| available(store) && store.read(Workbench::can_quit),
            |store| {
                if store.save_before_exit() {
                    nsh_core::element::desktop::quit();
                }
            },
        )
        .shortcut("mod+q"),
    );
    M::submenu("ChipDock", items)
}

fn serial_targets(store: Store) -> Vec<M> {
    vec![
        icon(
            item(
                store,
                "刷新串口",
                |store| available(store) && !store.read(|s| s.lane_busy(Lane::Discovery)),
                |store| store.command(|_| Command::Ports),
            )
            .shortcut("mod+r"),
            "refresh",
        ),
        icon(
            item(
                store,
                if serial_connection_active(store) {
                    "断开串口"
                } else {
                    "连接串口"
                },
                |store| {
                    available(store)
                        && store.read(|s| {
                            !s.serial_busy()
                                && s.active.is_some()
                                && (serial_status_active(s)
                                    || (!s.preferences.get("port").is_empty()
                                        && crate::backend::serial_config(&s.preferences).is_ok()))
                        })
                },
                |store| store.change(Workbench::connect_serial),
            )
            .shortcut("mod+k"),
            "cable",
        ),
        item(
            store,
            "断开串口",
            |store| {
                available(store)
                    && serial_connection_active(store)
                    && !store.read(Workbench::serial_busy)
            },
            |store| {
                store.change(|s| {
                    if let Some(id) = s.active {
                        s.submit(Command::Disconnect(id))
                    }
                })
            },
        )
        .shortcut("mod+shift+k"),
    ]
}
fn serial_status_active(s: &Workbench) -> bool {
    s.active_session()
        .and_then(|session| session.status.as_ref())
        .is_some_and(|status| status.connected || status.reconnecting)
}
fn serial_connection_active(store: Store) -> bool {
    store.read(serial_status_active)
}
fn flash_targets(store: Store, mode: Signal<&'static str>) -> Vec<M> {
    let mut items = vec![
        icon(
            item(store, "目标配置…", probe_idle, move |store| {
                mode.set("flash");
                store.target_chips.set(false);
                store.target_setup.set(true);
            }),
            "antenna",
        ),
        M::separator(),
        icon(
            item(
                store,
                "扫描探针",
                |store| probe_idle(store) || target_sheet(store),
                |store| store.command(|_| Command::Scan),
            )
            .shortcut("mod+r"),
            "refresh",
        ),
        icon(
            item(store, "连接目标", can_connect_target, |store| {
                if !target::connect_from_menu(store) {
                    store.command(|s| Command::Connect(s.preferences.clone()));
                }
            })
            .shortcut("mod+k"),
            "link",
        ),
        item(
            store,
            "断开目标",
            |store| probe_idle(store) && store.read(|s| s.target.is_some()),
            |store| store.change(Workbench::disconnect_target),
        )
        .shortcut("mod+shift+k"),
        M::separator(),
        item(
            store,
            "选择芯片…",
            |store| probe_idle(store) || target_sheet(store),
            move |store| {
                mode.set("flash");
                store.target_chips.set(true);
                store.target_setup.set(true);
            },
        ),
        item(store, "保存配置…", probe_idle, move |store| {
            mode.set("flash");
            auxiliary::open(store, auxiliary::Panel::Profiles);
        })
        .shortcut("mod+shift+s"),
    ];
    let profiles = store.read(|s| s.preferences.profiles.keys().cloned().collect::<Vec<_>>());
    items.push(
        M::submenu(
            "删除配置",
            profiles.iter().map(|name| {
                let name = name.clone();
                item(store, name.clone(), probe_idle, move |store| {
                    remove_profile(store, &name)
                })
            }),
        )
        .disabled(!probe_idle(store) || profiles.is_empty()),
    );
    if !profiles.is_empty() {
        items.push(
            M::submenu(
                "载入配置",
                profiles.into_iter().map(|name| {
                    item(store, name.clone(), probe_idle, move |store| {
                        store.change(|s| s.apply_profile(&name))
                    })
                }),
            )
            .disabled(!probe_idle(store)),
        );
    }
    items
}
pub(super) fn remove_profile(store: Store, name: &str) {
    let result = store.change(|s| {
        let before = s.preferences.clone();
        s.preferences.profiles.remove(name);
        if let Err(error) = s.save() {
            s.preferences = before.clone();
            if let Some(session) = s.active.and_then(|id| s.sessions.get_mut(&id)) {
                session.preferences = before;
            }
            return Err(error);
        }
        Ok(())
    });
    if let Err(error) = result {
        store.report(error)
    }
}
fn log_actions(store: Store) -> Vec<M> {
    vec![
        item(
            store,
            "仅显示错误",
            |_| true,
            |store| toggle_flag(store, "errors_only"),
        )
        .checked(store.flag("errors_only")),
        item(
            store,
            "跟随最新日志",
            |_| true,
            |store| toggle_flag(store, "follow_logs"),
        )
        .checked(store.flag("follow_logs")),
        item(store, "复制全部日志", has_logs, |store| {
            set_clipboard(store.read(|s| s.logs.iter().cloned().collect::<Vec<_>>().join("\n")));
        }),
        item(store, "清空日志", has_logs, |store| {
            store.change(|s| s.logs.clear())
        }),
    ]
}
fn serial_actions(store: Store) -> Vec<M> {
    vec![
        icon(
            item(
                store,
                "发送数据",
                |store| available(store) && store.read(can_send),
                |store| store.change(Workbench::send),
            )
            .shortcut("mod+enter"),
            "send",
        ),
        icon(
            item(
                store,
                "清空终端",
                |store| {
                    available(store)
                        && store.read(|s| {
                            s.active_session()
                                .is_some_and(|session| session.records().next().is_some())
                        })
                },
                |store| store.change(Workbench::clear_terminal),
            )
            .shortcut("mod+l"),
            "trash",
        ),
        icon(
            item(
                store,
                if repeating(store) {
                    "停止自动发送"
                } else {
                    "开始自动发送"
                },
                |store| {
                    available(store)
                        && (repeating(store)
                            || store.read(|s| {
                                can_send(s)
                                    && !s.preferences.flag("command_mode")
                                    && s.preferences
                                        .number("interval")
                                        .is_ok_and(|value| (10..=3_600_000).contains(&value))
                            }))
                },
                |store| {
                    let stopping = repeating(store);
                    store
                        .change(|s| s.serial_action(if stopping { "cancelJob" } else { "repeat" }));
                },
            )
            .shortcut("mod+alt+enter"),
            if repeating(store) { "stop" } else { "repeat" },
        ),
        item(store, "自动滚动", available, |store| {
            toggle_flag(store, "auto_scroll")
        })
        .checked(store.flag("auto_scroll")),
    ]
}
fn recent_menu(store: Store, mode: Signal<&'static str>) -> M {
    let recent = store.read(|s| s.preferences.recent.clone());
    let missing = recent
        .iter()
        .any(|path| !std::path::Path::new(path).is_file());
    let mut items = recent
        .iter()
        .map(|path| {
            let path = path.clone();
            let exists = std::path::Path::new(&path).is_file();
            let name = std::path::Path::new(&path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let selected_path = path.clone();
            item(
                store,
                format!("{name}{}", if exists { "" } else { "（文件不存在）" }),
                move |store| probe_idle(store) && std::path::Path::new(&path).is_file(),
                {
                    let path = selected_path;
                    move |store| {
                        mode.set("flash");
                        store.change(|s| s.open_recent(&path));
                    }
                },
            )
            .disabled(!probe_idle(store) || !exists)
        })
        .collect::<Vec<_>>();
    items.push(M::separator());
    items.push(M::submenu(
        "移除记录",
        recent.iter().map(|path| {
            let path = path.clone();
            let name = std::path::Path::new(&path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            item(store, name, probe_idle, move |store| {
                firmware_history::remove_one(store, &path)
            })
        }),
    ));
    items.push(item(
        store,
        "清除失效记录",
        move |store| probe_idle(store) && missing,
        firmware_history::remove_missing,
    ));
    items.push(item(
        store,
        "清空最近使用",
        probe_idle,
        firmware_history::remove_all,
    ));
    M::submenu("打开最近使用的", items).disabled(!probe_idle(store) || recent.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{follow_menu_state, valid_memory_range};
    use nsh_core::element::reactive::{Runtime, cleanup, create_root, signal};
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn polling_preserves_menu_handlers_until_visible_command_state_changes() {
        let runtime = Runtime::default();
        let _entered = runtime.enter();
        let active = Rc::new(Cell::new(false));
        let registrations = Rc::new(Cell::new(0));
        let (owner, (revision, enabled)) = create_root({
            let active = active.clone();
            let registrations = registrations.clone();
            move || {
                let revision = signal(0);
                let enabled = signal(true);
                follow_menu_state(
                    move || {
                        revision.get();
                        enabled.get()
                    },
                    move || {
                        registrations.set(registrations.get() + 1);
                        active.set(true);
                        let active = active.clone();
                        cleanup(move || active.set(false));
                    },
                );
                (revision, enabled)
            }
        });
        for tick in 1..=100 {
            revision.set(tick);
        }
        assert!(active.get());
        assert_eq!(registrations.get(), 1);
        enabled.set(false);
        assert!(active.get());
        assert_eq!(registrations.get(), 2);
        revision.set(101);
        assert!(active.get());
        assert_eq!(registrations.get(), 2);
        owner.dispose();
        assert!(!active.get());
    }

    #[test]
    fn memory_commands_reject_zero_oversized_and_overflowing_ranges() {
        assert!(valid_memory_range(0x0800_0000, 4096));
        assert!(valid_memory_range(u64::MAX - 1, 1));
        assert!(!valid_memory_range(0x0800_0000, 0));
        assert!(!valid_memory_range(0x0800_0000, 4097));
        assert!(!valid_memory_range(u64::MAX, 1));
        assert!(!valid_memory_range(u64::MAX - 4095, 4096));
    }
}
