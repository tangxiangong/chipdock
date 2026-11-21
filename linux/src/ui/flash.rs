use super::*;
use chipdock_core::models::{FirmwareFormat, MemoryRegionKind};
use nsh_base::alert_dialog;

/// Virtualized byte rows, kept with the firmware buffer in Rust.
#[component]
pub fn FirmwareBytes() -> impl IntoElement {
    let (store, _) = workbench_context();
    let preview = computed(move || {
        store.read(|s| {
            s.preview
                .as_ref()
                .map(|p| (p.start_address, p.bytes.clone()))
        })
    });
    let rows = computed(move || {
        preview.with(|p| {
            p.as_ref()
                .map(|(start, bytes)| {
                    bytes
                        .chunks(16)
                        .enumerate()
                        .map(|(i, bytes)| {
                            (
                                format!("0x{:08X}", start + (i * 16) as u64),
                                bytes
                                    .iter()
                                    .map(|b| b.map_or("··".into(), |b| format!("{b:02X}")))
                                    .collect::<Vec<_>>()
                                    .join(" "),
                                bytes
                                    .iter()
                                    .map(|b| {
                                        b.filter(|b| (32..127).contains(b)).map_or('·', char::from)
                                    })
                                    .collect::<String>(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    view()
        .flex_1()
        .min_h_0()
        .border_1()
        .border_color(hairline)
        .rounded(move || theme().radius)
        .child(
            view()
                .h(px(25.))
                .px(px(11.))
                .flex_row()
                .items_center()
                .gap_3()
                .text_color(move || theme().muted_foreground)
                .bg(move || theme().color("well").unwrap_or(theme().muted))
                .border_b_1()
                .border_color(hairline)
                .child(text("地址").w(px(80.)).text_size(small_size))
                .child(text("十六进制").w(px(376.)).text_size(small_size))
                .child(text("ASCII").text_size(small_size)),
        )
        .child(
            virtual_list(
                move || rows.with(Vec::len),
                move |at| {
                    view()
                        .h(px(17.))
                        .px(px(11.))
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .font_family(mono)
                        .text_size(caption_size)
                        .child(
                            text(move || {
                                rows.with(|r| {
                                    r.get(at.index()).map(|r| r.0.clone()).unwrap_or_default()
                                })
                            })
                            .w(px(80.))
                            .text_color(move || theme().muted_foreground),
                        )
                        .child(
                            text(move || {
                                rows.with(|r| {
                                    r.get(at.index()).map(|r| r.1.clone()).unwrap_or_default()
                                })
                            })
                            .w(px(376.)),
                        )
                        .child(
                            text(move || {
                                rows.with(|r| {
                                    r.get(at.index()).map(|r| r.2.clone()).unwrap_or_default()
                                })
                            })
                            .text_color(move || theme().muted_foreground),
                        )
                },
            )
            .estimated_item_size(px(17.))
            .content_padding_top(px(3.))
            .flex_1(),
        )
}
fn firmware_menu(store: Store) -> impl IntoElement {
    use nsh_core::element::desktop::MenuItem;
    view()
        .size(px(24.))
        .items_end()
        .justify_center()
        .child(native_borderless_menu_disabled(
            view()
                .w(px(21.))
                .h(px(15.))
                .items_center()
                .justify_center()
                .label("固件操作")
                .child(icons::icon("more")),
            move || store.locked("firmware"),
            move || {
                vec![
                    MenuItem::new("更换固件…").on_select(move || store.pick("firmware")),
                    MenuItem::new(if cfg!(target_os = "macos") {
                        "在访达中显示"
                    } else {
                        "在文件夹中显示"
                    })
                    .on_select(move || {
                        let path = store.field("firmware");
                        store.spawn(async move {
                            if let Err(error) = reveal_path(path).await {
                                store.report(error);
                            }
                        });
                    }),
                    MenuItem::separator(),
                    MenuItem::new("关闭固件").on_select(move || {
                        store.change(|state| state.set("firmware", String::new()))
                    }),
                ]
            },
        ))
}

fn layout_bar(store: Store) -> View {
    let extent = computed(move || {
        store.read(|s| {
            let Some(p) = &s.preview else {
                return (0, 1, 0, 0, vec![]);
            };
            let u = &p.usage;
            let used_start = u.start_address.unwrap_or(0);
            let used_end = u.end_address.unwrap_or(used_start);
            let flash = s
                .layout
                .iter()
                .filter(|r| matches!(r.kind, MemoryRegionKind::Nvm) && !r.is_alias)
                .filter(|r| r.start < used_end && r.end > used_start)
                .collect::<Vec<_>>();
            let start = flash
                .iter()
                .map(|r| r.start)
                .min()
                .unwrap_or(used_start)
                .min(used_start);
            let end = flash
                .iter()
                .map(|r| r.end)
                .max()
                .unwrap_or(used_end)
                .max(used_end.max(1));
            (
                start,
                end,
                flash.iter().map(|r| r.size).sum::<u64>(),
                u.used_bytes,
                u.segments
                    .iter()
                    .map(|s| (s.start, s.size))
                    .collect::<Vec<_>>(),
            )
        })
    });
    view()
        .gap(px(6.))
        .child(
            view()
                .relative()
                .h(px(8.))
                .rounded(px(3.))
                .overflow_hidden()
                .bg(move || theme().color("well").unwrap_or(theme().muted))
                .child(each(
                    computed(move || extent.with(|e| e.4.clone())),
                    |r: &(u64, u64)| r.0,
                    move |segment, _| {
                        view()
                            .absolute()
                            .h_full()
                            .left(move || {
                                percentage(extent.with(|e| {
                                    (segment.get().0 - e.0) as f32 / (e.1 - e.0).max(1) as f32
                                }))
                            })
                            .w(move || {
                                percentage(
                                    extent.with(|e| {
                                        segment.get().1 as f32 / (e.1 - e.0).max(1) as f32
                                    }),
                                )
                            })
                            .min_w(px(2.))
                            .bg(move || theme().primary)
                    },
                )),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .font_family(mono)
                .text_size(small_size)
                .text_color(move || theme().muted_foreground)
                .child(text(move || extent.with(|e| format!("0x{:08X}", e.0))))
                .child(view().flex_1())
                .child(text(move || {
                    extent.with(|e| {
                        if e.2 > 0 {
                            format!(
                                "Flash {} · 已用 {:.1}%",
                                byte_count(e.2),
                                e.3 as f64 / e.2 as f64 * 100.
                            )
                        } else {
                            String::new()
                        }
                    })
                }))
                .child(view().flex_1())
                .child(text(move || {
                    extent.with(|e| format!("0x{:08X}", e.1.saturating_sub(1)))
                })),
        )
}
fn abbreviate(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    if let Ok(home) = std::env::var("HOME")
        && let Some(suffix) = path.strip_prefix(&home)
    {
        format!("~{suffix}")
    } else {
        path.into_owned()
    }
}
fn byte_count(bytes: u64) -> String {
    if bytes >= 1048576 {
        format!("{:.1} MiB", bytes as f64 / 1048576.)
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.)
    } else {
        format!("{bytes} B")
    }
}
pub(super) fn execute(store: Store, preflight: bool) {
    if store.read(|s| {
        !s.target_connected()
            || s.lane_busy(Lane::Probe)
            || s.preferences.get("firmware").is_empty()
    }) {
        return;
    }
    let mut p = store.read(|s| s.preferences.clone());
    if preflight {
        p.flags.insert("dry_run".into(), true);
    }
    if p.get("erase") == "full" && !p.flag("dry_run") {
        store.spawn(async move {
            let result = alert_dialog()
                .title(format!("擦除并烧录 {}？", p.get("chip")))
                .description("整片擦除会永久删除目标上的全部固件和数据。")
                .cancel("cancel", "取消")
                .destructive("flash", "擦除并烧录")
                .show()
                .await;
            if matches!(result,Ok(r) if r.action.as_deref()==Some("flash"))
                && store
                    .read(|s| s.target_connected() && s.preferences.target_key() == p.target_key())
            {
                store.command(|_| Command::Flash(p));
            }
        });
    } else {
        store.command(|_| Command::Flash(p));
    }
}
pub(super) fn confirm_erase(store: Store) {
    if store.read(|state| !state.target_connected() || state.lane_busy(Lane::Probe)) {
        return;
    }
    let p = store.read(|s| s.preferences.clone());
    store.spawn(async move {
        let result = alert_dialog()
            .title(format!("擦除 {}？", p.get("chip")))
            .description("目标的全部固件和数据将被永久删除。")
            .cancel("cancel", "取消")
            .destructive("erase", "擦除")
            .show()
            .await;
        if matches!(result,Ok(r) if r.action.as_deref()==Some("erase"))
            && store.read(|s| s.target_connected() && s.preferences.target_key() == p.target_key())
        {
            store.command(|_| Command::Erase(p));
        }
    });
}

#[derive(Clone, Debug, PartialEq, Props)]
pub struct FirmwareContentProps {
    /// The TSX document or options layout.
    pub children: Prop<Children>,
}

#[derive(Debug, Clone, Default, PartialEq, JsObject)]
pub struct FirmwareDetails {
    pub name: String,
    pub directory: String,
    pub format: String,
    pub size: String,
    pub segments: String,
    pub range: String,
    pub preview_range: String,
    pub error: Option<String>,
    pub analyzing: bool,
}

#[derive(Clone, Debug, PartialEq, Event)]
pub enum FirmwareDocumentEvent {
    /// The selected file's display metadata. Byte buffers remain in the virtual table.
    #[args(details)]
    Change(FirmwareDetails),
}

/// Reports metadata from the current firmware analysis.
#[component]
pub fn FirmwareDocumentHost(
    props: FirmwareContentProps,
    cx: Scope<FirmwareDocumentEvent>,
) -> impl IntoElement {
    let (store, _) = workbench_context();
    let details = computed(move || {
        store.read(|state| {
            let path = std::path::Path::new(state.preferences.get("firmware"));
            let mut details = FirmwareDetails {
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                directory: abbreviate(path.parent().unwrap_or_else(|| std::path::Path::new(""))),
                error: state.firmware_error.clone(),
                analyzing: state.firmware_analyzing(),
                ..Default::default()
            };
            if let Some(preview) = &state.preview {
                let usage = &preview.usage;
                details.format = match usage.format {
                    FirmwareFormat::Elf => "ELF",
                    FirmwareFormat::Hex => "HEX",
                    FirmwareFormat::Bin => "BIN",
                }
                .into();
                details.size = byte_count(usage.used_bytes);
                details.segments = usage.segments.len().to_string();
                details.range = format!(
                    "0x{:08X} – 0x{:08X}",
                    usage.start_address.unwrap_or(0),
                    usage.end_address.unwrap_or(1).saturating_sub(1)
                );
                details.preview_range = format!(
                    "0x{:08X} – 0x{:08X}",
                    preview.start_address,
                    preview.start_address + preview.bytes.len().saturating_sub(1) as u64
                );
            }
            details
        })
    });
    effect(move || cx.emit(FirmwareDocumentEvent::Change(details.get())));
    view().child(props.children)
}

/// Native menu of file commands with the workbench's file dialog owner.
#[component]
pub fn FirmwareFileMenu() -> impl IntoElement {
    let (store, _) = workbench_context();
    firmware_menu(store)
}

/// Draws the firmware segments against the target memory regions.
#[component]
pub fn FirmwareMemoryMap() -> impl IntoElement {
    let (store, _) = workbench_context();
    layout_bar(store)
}

#[derive(Debug, Clone, Default, PartialEq, JsObject)]
pub struct FlashState {
    pub connected: bool,
    pub busy: bool,
    pub empty: bool,
    pub dry_run: bool,
    pub target: String,
    pub chip: String,
    pub wire: String,
    pub speed: String,
    pub probe: String,
    pub erase: String,
}

#[derive(Clone, Debug, PartialEq, Event)]
pub enum FlashOptionsEvent {
    /// The connection and flash options shown in this panel.
    #[args(state)]
    Change(FlashState),
}

/// Provides flash option values to the scrollable TSX form.
#[component]
pub fn FlashOptionsHost(
    props: FirmwareContentProps,
    cx: Scope<FlashOptionsEvent>,
) -> impl IntoElement {
    let (store, _) = workbench_context();
    let state = computed(move || {
        store.read(|state| {
            let prefs = &state.preferences;
            FlashState {
                connected: state.target_connected(),
                busy: state.lane_busy(Lane::Probe),
                empty: prefs.get("firmware").is_empty(),
                dry_run: prefs.flag("dry_run"),
                target: state.target_name(),
                chip: prefs.get("chip").into(),
                wire: prefs.get("wire").to_uppercase(),
                speed: prefs.get("speed").into(),
                probe: prefs.get("probe").into(),
                erase: prefs.get("erase").into(),
            }
        })
    });
    effect(move || cx.emit(FlashOptionsEvent::Change(state.get())));
    scroll().flex_1().child(props.children)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PropEnum)]
pub enum FlashCommand {
    Disconnect,
    Setup,
    Erase,
    #[default]
    Program,
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct FlashButtonProps {
    /// The guarded workbench operation.
    pub command: Prop<FlashCommand>,
    /// The current localized action label.
    pub label: Prop<SharedString>,
}

/// Runs a flash action using the existing target/session checks.
#[component]
pub fn FlashButton(props: FlashButtonProps) -> impl IntoElement {
    let (store, _) = workbench_context();
    let command = props.command.get();
    let icon = match command {
        FlashCommand::Disconnect => "close",
        FlashCommand::Setup => "settings",
        FlashCommand::Erase => "erase",
        FlashCommand::Program => "link",
    };
    labeled_button(
        icon,
        props.label,
        command == FlashCommand::Program,
        move || {
            store.read(|s| {
                s.lane_busy(Lane::Probe)
                    || match command {
                        FlashCommand::Erase => !s.target_connected(),
                        FlashCommand::Program => {
                            s.target_connected() && s.preferences.get("firmware").is_empty()
                        }
                        _ => false,
                    }
            })
        },
        move || match command {
            FlashCommand::Disconnect => store.change(Workbench::disconnect_target),
            FlashCommand::Setup => store.target_setup.set(true),
            FlashCommand::Erase => confirm_erase(store),
            FlashCommand::Program if store.read(Workbench::target_connected) => {
                execute(store, false)
            }
            FlashCommand::Program => store.command(|s| Command::Connect(s.preferences.clone())),
        },
    )
}

/// Native erase mode menu using the persisted workbench preference.
#[component]
pub fn EraseModePicker() -> impl IntoElement {
    let (store, _) = workbench_context();
    popup_control(store, "erase", &["automatic", "full", "skip"], false)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PropEnum)]
pub enum FlashFlag {
    #[default]
    Verify,
    Reset,
    DryRun,
}
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FlashToggleProps {
    /// Flash option changed by this control.
    pub option: Prop<FlashFlag>,
}

/// Updates the existing flash preference with the shared checkbox behavior.
#[component]
pub fn FlashToggle(props: FlashToggleProps) -> impl IntoElement {
    let (store, _) = workbench_context();
    let (key, label) = match props.option.get() {
        FlashFlag::Verify => ("verify", "校验写入的数据"),
        FlashFlag::Reset => ("reset", "复位并运行程序"),
        FlashFlag::DryRun => ("dry_run", "仅预检查，不写入芯片"),
    };
    toggle(store, key, label)
}

pub(super) fn register(registry: &mut Registry) {
    registry
        .register::<FirmwareDocumentHost>()
        .register::<FirmwareBytes>()
        .register::<FirmwareFileMenu>()
        .register::<FirmwareMemoryMap>()
        .register::<FlashOptionsHost>()
        .register::<FlashButton>()
        .register::<EraseModePicker>()
        .register::<FlashToggle>();
}
