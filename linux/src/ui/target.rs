use super::*;
use crate::model::Preferences;
use nsh_base::{
    DataTableCell, DataTableClasses, DataTableColumn, DataTablePress, DataTableRows,
    DataTableSelection, SelectItem, data_table,
};

#[derive(Debug, Clone, PartialEq, Props)]
pub struct TargetContentProps {
    /// The TSX configuration form.
    pub children: Prop<Children>,
}

#[derive(Clone, Copy)]
struct TargetDraft {
    store: Store,
    draft: Signal<Preferences>,
}

fn editor() -> TargetDraft {
    use_context().expect("target configuration owns its draft")
}

/// Owns the editable target, cancellation policy and native chip search table.
#[component]
pub fn TargetDialogHost(props: TargetContentProps) -> impl IntoElement {
    let (store, _) = workbench_context();
    let open = store.target_setup;
    provide_context(ModalControls);
    let draft = signal_with(store.read(|s| s.preferences.clone()), SignalEquals::Never);
    provide_context(TargetDraft { store, draft });
    store.target_draft.replace(Some(draft));
    let chips = store.target_chips;
    let standalone_chip_picker = chips.get_untracked();
    let selected = signal(Some(draft.with_untracked(|p| p.get("chip").to_owned())));
    let was_connecting = signal(false);
    effect(move || {
        if store.read(Workbench::target_connecting) {
            was_connecting.set(true);
        } else if was_connecting.get() {
            was_connecting.set(false);
            if store.read(Workbench::target_connected) {
                open.set(false);
            } else if store.read(|state| !state.chips.is_empty()) {
                chips.set(true);
            }
        }
    });
    store.modal.set(true);
    cleanup(move || {
        store.target_draft.replace(None);
        store.modal.set(false);
        store.target_chips.set(false);
    });
    nsh_base::dialog()
        .label(move || {
            if chips.get() {
                "选择芯片"
            } else {
                "目标配置"
            }
        })
        .on_cancel(move || {
            if !store.read(|state| state.lane_busy(Lane::Probe)) {
                if standalone_chip_picker {
                    open.set(false);
                } else if chips.get_untracked() {
                    chips.set(false);
                } else {
                    open.set(false);
                }
            }
        })
        .flex_1()
        .items_center()
        .justify_center()
        .bg(move || modal_backdrop(store))
        .text_color(move || theme().foreground)
        .child(
            view()
                .rounded(px(16.))
                .overflow_hidden()
                .bg(move || theme().background)
                .child(
                    when(chips, move || {
                        chip_picker(store, draft, selected, chips, open, standalone_chip_picker)
                    })
                    .otherwise(move || props.children),
                ),
        )
}
fn valid_target(draft: &Preferences) -> bool {
    valid_speed(draft)
        && draft.target().is_ok()
        && (draft.get("base").trim().is_empty() || crate::model::number(draft.get("base")).is_ok())
}

pub(super) fn can_connect_from_menu(store: Store) -> bool {
    store
        .target_draft
        .with(|draft| draft.is_some_and(|draft| draft.with(valid_target)))
        && store.read(|state| {
            !state.lane_busy(Lane::Probe) && (!state.probes_scanned || !state.probes.is_empty())
        })
}

pub(super) fn connect_from_menu(store: Store) -> bool {
    let Some(draft) = store.target_draft.get() else {
        return false;
    };
    if can_connect_from_menu(store) {
        let preferences = draft.get_untracked();
        store.change(|state| {
            for key in ["probe", "chip", "wire", "speed", "base"] {
                state.set(key, preferences.get(key).into());
            }
            state.flag("under_reset", preferences.flag("under_reset"));
            state.submit(Command::Connect(state.preferences.clone()));
        });
    }
    // A live draft consumes the command even when invalid: never connect with
    // the saved preferences behind a target configuration window.
    true
}

#[derive(Debug, Clone, Default, PartialEq, JsObject)]
pub struct TargetFormState {
    pub busy: bool,
    pub connected: bool,
    pub name: String,
    pub scanned: bool,
    pub probe_count: u32,
    pub chip: String,
    pub wire: String,
    pub valid_speed: bool,
    pub under_reset: bool,
    pub valid_base: bool,
    pub inferred: Option<String>,
    pub identified: bool,
}
#[derive(Debug, Clone, PartialEq, Event)]
pub enum TargetFormEvent {
    /// Changes to the draft and the current probe state.
    #[args(state)]
    Change(TargetFormState),
}

pub struct TargetFormApi(TargetDraft);
#[methods]
impl TargetFormApi {
    /// Scans probes for the active configuration draft.
    pub fn scan(&self) {
        let store = self.0.store;
        if !store.read(|s| s.lane_busy(Lane::Probe)) {
            store.command(|_| Command::Scan);
        }
    }
    /// Opens chip search while retaining this configuration.
    pub fn select_chip(&self) {
        if !self.0.store.read(|s| s.lane_busy(Lane::Probe)) {
            self.0.store.target_chips.set(true);
        }
    }
    /// Changes the wire protocol in the draft.
    pub fn set_wire(&self, wire: String) {
        if matches!(wire.as_str(), "swd" | "jtag")
            && !self.0.store.read(|s| s.lane_busy(Lane::Probe))
        {
            self.0.draft.update(|p| {
                p.fields.insert("wire".into(), wire);
            });
        }
    }
    /// Changes connect-under-reset for the selected target.
    pub fn set_under_reset(&self, enabled: bool) {
        if !self.0.store.read(|s| s.lane_busy(Lane::Probe)) {
            self.0.draft.update(|p| {
                p.flags.insert("under_reset".into(), enabled);
            });
        }
    }
}

/// Exposes target configuration state without moving probe operations or keyboard commands into JS.
#[component(methods = TargetFormApi)]
pub fn TargetFormHost(props: TargetContentProps, cx: Scope<TargetFormEvent>) -> impl IntoElement {
    let current = editor();
    let TargetDraft { store, draft } = current;
    cx.methods(TargetFormApi(current));
    on_key_with(
        "enter",
        KeyOptions {
            when: nsh_core::keymap::When::NotEditing,
            ..Default::default()
        },
        move |_| {
            connect_from_menu(store);
        },
    );

    let state = computed(move || {
        let inferred = inferred_bin_address(store, draft);
        draft.with(|p| {
            store.read(|s| TargetFormState {
                busy: s.lane_busy(Lane::Probe),
                connected: s.target_connected(),
                name: s.target_name(),
                scanned: s.probes_scanned,
                probe_count: s.probes.len() as u32,
                chip: p.get("chip").into(),
                wire: p.get("wire").into(),
                valid_speed: valid_speed(p),
                under_reset: p.flag("under_reset"),
                valid_base: valid_base(p),
                inferred: inferred.then(|| {
                    s.preview
                        .as_ref()
                        .and_then(|p| p.usage.start_address)
                        .map(|a| format!("0x{a:08X}"))
                        .unwrap_or_default()
                }),
                identified: s.target_information().is_some(),
            })
        })
    });
    effect(move || cx.emit(TargetFormEvent::Change(state.get())));
    view().child(props.children)
}

/// Probe list with the existing native selector trigger and item styling.
#[component]
pub fn TargetProbePicker() -> impl IntoElement {
    let TargetDraft { store, draft } = editor();
    let busy = move || store.read(|s| s.lane_busy(Lane::Probe));
    nsh_base::select()
        .items(move || {
            store.read(|state| {
                std::iter::once(SelectItem::new("", "自动选择"))
                    .chain(state.probes.iter().map(|probe| {
                        SelectItem::new(
                            probe.identifier.clone(),
                            probe
                                .product
                                .clone()
                                .unwrap_or_else(|| probe.identifier.clone()),
                        )
                    }))
                    .collect::<Vec<_>>()
            })
        })
        .value(move || {
            Some(SharedString::from(
                draft.with(|value| value.get("probe").to_owned()),
            ))
        })
        .on_change(move |value: &Option<SharedString>| {
            if let Some(value) = value {
                draft.update(|draft| {
                    draft.fields.insert("probe".into(), value.to_string());
                });
            }
        })
        .disabled(busy)
        .popup_style(popup)
        .accessibility_label("调试探针")
        .render_item(|item: &nsh_base::SelectEntry| {
            text(item.title.clone()).text_size(body_size).px_2().py_1()
        })
        .trigger(
            view()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .child(text(move || probe_name(store, draft)).text_size(body_size))
                .child(
                    view()
                        .size(px(20.))
                        .rounded_full()
                        .bg(move || theme().foreground.opacity(0.06))
                        .items_center()
                        .justify_center()
                        .child(icons::icon("target-select").size(px(10.))),
                ),
        )
}

/// Connection speed editor, including select-all on its first focus.
#[component]
pub fn TargetSpeedInput() -> impl IntoElement {
    let TargetDraft { store, draft } = editor();
    let busy = move || store.read(|s| s.lane_busy(Lane::Probe));
    let speed_length =
        draft.with_untracked(|value| value.get("speed").encode_utf16().count() as u32);
    let speed_selection = signal((0_u32, 0_u32));
    let selected_speed = stored(false);
    nsh_base::input()
        .label("速度")
        .value(move || draft.with(|value| value.get("speed").to_owned()))
        .on_change(move |value: &str| {
            draft.update(|draft| {
                draft.fields.insert("speed".into(), value.into());
            })
        })
        .on_submit(move |_: &str| {
            connect_from_menu(store);
        })
        .auto_focus(true)
        .on_focus(move || {
            if !selected_speed.replace(true) {
                speed_selection.set((0, speed_length));
            }
        })
        .selected_range(speed_selection)
        .readonly(busy)
        .w(px(80.))
        .h(px(16.))
        .text_size(body_size)
        .text_right()
        .tabular_nums()
        .caret_color(|| color(theme().primary))
        .selection_color(|| color(theme().primary.opacity(0.4)))
}
/// Platform menu for standard connection speeds.
#[component]
pub fn TargetSpeedMenu() -> impl IntoElement {
    let TargetDraft { store, draft } = editor();
    let busy = move || store.read(|s| s.lane_busy(Lane::Probe));
    native_menu_disabled(
        view()
            .size(px(12.))
            .items_center()
            .justify_center()
            .label("常用连接速度")
            .child(icons::icon("target-down").size(px(12.))),
        busy,
        move || {
            [100, 1000, 4000, 10000, 24000]
                .into_iter()
                .map(move |speed| {
                    nsh_core::element::desktop::MenuItem::new(format!("{speed} kHz")).on_select(
                        move || {
                            draft.update(|value| {
                                value.fields.insert("speed".into(), speed.to_string());
                            })
                        },
                    )
                })
                .collect()
        },
    )
}
/// Hexadecimal address editor sharing draft validation with the connection command.
#[component]
pub fn TargetBaseInput() -> impl IntoElement {
    let TargetDraft { store, draft } = editor();
    let busy = move || store.read(|s| s.lane_busy(Lane::Probe));
    nsh_base::input()
        .label("BIN 基地址")
        .placeholder("自动识别")
        .value(move || draft.with(|value| value.get("base").to_owned()))
        .on_change(move |value: &str| {
            draft.update(|draft| {
                draft.fields.insert("base".into(), value.into());
            })
        })
        .on_submit(move |_: &str| {
            connect_from_menu(store);
        })
        .readonly(busy)
        .flex_1()
        .h(px(16.))
        .text_right()
        .text_size(body_size)
        .font_family(mono)
        .placeholder_color(|| color(theme().muted_foreground))
        .caret_color(|| color(theme().primary))
}
/// Identification information reported by the connected debug target.
#[component]
pub fn TargetIdentification() -> impl IntoElement {
    identified_information(editor().store)
}
/// Connects using the live draft and retains the native default-button behavior.
#[component]
pub fn TargetConnect() -> impl IntoElement {
    target_connect_button(editor().store)
}
/// Closes the target draft while idle.
#[component]
pub fn TargetCancel() -> impl IntoElement {
    let store = editor().store;
    action(
        "取消",
        Variant::Default,
        move || store.read(|s| s.lane_busy(Lane::Probe)),
        move || store.target_setup.set(false),
    )
}
/// Disconnects the selected device from the native workbench.
#[component]
pub fn TargetDisconnect() -> impl IntoElement {
    let store = editor().store;
    action(
        "断开",
        Variant::Ghost,
        move || store.read(|s| s.lane_busy(Lane::Probe)),
        move || store.change(Workbench::disconnect_target),
    )
}
pub(super) fn register(registry: &mut Registry) {
    registry
        .register::<TargetDialogHost>()
        .register::<TargetFormHost>()
        .register::<TargetProbePicker>()
        .register::<TargetSpeedInput>()
        .register::<TargetSpeedMenu>()
        .register::<TargetBaseInput>()
        .register::<TargetIdentification>()
        .register::<TargetConnect>()
        .register::<TargetCancel>()
        .register::<TargetDisconnect>();
}

fn probe_name(store: Store, draft: Signal<Preferences>) -> String {
    let selected = draft.with(|draft| draft.get("probe").to_owned());
    if selected.is_empty() {
        return "自动选择".into();
    }
    store.read(|state| {
        state
            .probes
            .iter()
            .find(|probe| probe.identifier == selected)
            .and_then(|probe| probe.product.clone())
            .unwrap_or(selected)
    })
}

fn target_connect_button(store: Store) -> impl IntoElement {
    let disabled = computed(move || !can_connect_from_menu(store));
    let active = control_active();
    nsh_base::button()
        .disabled(disabled)
        .on_press(move || {
            connect_from_menu(store);
        })
        .w(move || {
            px(if store.read(Workbench::target_connected) {
                100.5
            } else {
                74.5
            })
        })
        .h(px(24.))
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(5.))
        .rounded(px(7.))
        .bg(move || {
            if active.get() {
                default_push_fill(disabled.get())
            } else {
                theme()
                    .color(if disabled.get() {
                        "primary-inactive-disabled"
                    } else {
                        "primary-inactive"
                    })
                    .unwrap_or(theme().secondary)
            }
        })
        .text_color(move || {
            if !active.get() {
                theme()
                    .color(if disabled.get() {
                        "control-inactive-disabled-foreground"
                    } else {
                        "control-inactive-foreground"
                    })
                    .unwrap_or(theme().muted_foreground)
            } else if disabled.get() {
                theme()
                    .color("default-push-disabled-foreground")
                    .unwrap_or(theme().muted_foreground)
            } else {
                theme().primary_foreground
            }
        })
        .hover(|style| style.bg(move || theme().primary.opacity(0.9)))
        .active(|style| style.bg(move || theme().primary.opacity(0.8)))
        .focus_visible(|style| style.outline(px(2.)).outline_color(move || theme().ring))
        .disabled_style(|style| style.opacity(1.))
        .child(icons::icon("link").size(px(14.)))
        .child(
            text(move || {
                store.read(|state| {
                    if state.target_connected() {
                        "重新连接"
                    } else {
                        "连接"
                    }
                })
            })
            .text_size(body_size),
        )
}

fn inferred_bin_address(store: Store, draft: Signal<Preferences>) -> bool {
    draft.with(|value| value.get("base").trim().is_empty())
        && store.read(|state| {
            state.preferences.get("base").trim().is_empty()
                && state.preview.as_ref().is_some_and(|preview| {
                    matches!(
                        preview.usage.format,
                        chipdock_core::models::FirmwareFormat::Bin
                    ) && preview.usage.start_address.is_some()
                })
        })
}

fn identified_information(store: Store) -> View {
    let has_cpu = move || {
        store.read(|state| {
            state
                .target_information()
                .is_some_and(|information| information.cpu.is_some())
        })
    };
    let has_flash = move || {
        store.read(|state| {
            state
                .target_information()
                .is_some_and(|information| information.flash_size_bytes.is_some())
        })
    };
    view().gap_2().child(target_section("已识别")).child(
        form_group()
            .child(
                target_row("型号", 36., has_cpu() || has_flash())
                    .child(view().flex_1())
                    .child(
                        text(move || {
                            store.read(|state| {
                                state
                                    .target_information()
                                    .map(|information| information.device_type.clone())
                                    .unwrap_or_default()
                            })
                        })
                        .text_size(body_size)
                        .text_color(move || theme().muted_foreground),
                    ),
            )
            .child(when(has_cpu, move || {
                target_row("内核", 36., has_flash())
                    .child(view().flex_1())
                    .child(
                        text(move || {
                            store.read(|state| {
                                state
                                    .target_information()
                                    .and_then(|information| information.cpu.clone())
                                    .unwrap_or_default()
                            })
                        })
                        .text_size(body_size)
                        .text_color(move || theme().muted_foreground),
                    )
            }))
            .child(when(has_flash, move || {
                target_row("Flash", 36., false)
                    .child(view().flex_1())
                    .child(
                        text(move || {
                            store.read(|state| {
                                state
                                    .target_information()
                                    .and_then(|information| information.flash_size_bytes)
                                    .map(|bytes| {
                                        if bytes >= 1_048_576 {
                                            format!("{:.1} MiB", bytes as f64 / 1_048_576.)
                                        } else if bytes >= 1024 {
                                            format!("{:.1} KiB", bytes as f64 / 1024.)
                                        } else {
                                            format!("{bytes} B")
                                        }
                                    })
                                    .unwrap_or_default()
                            })
                        })
                        .text_size(body_size)
                        .text_color(move || theme().muted_foreground),
                    )
            })),
    )
}

fn target_section(label: &'static str) -> View {
    view().h(px(16.)).px(px(10.)).justify_center().child(
        text(label)
            .text_size(body_size)
            .font_weight(FontWeight::BOLD),
    )
}

fn target_row(label: &'static str, height: f32, separator: bool) -> View {
    view()
        .h(px(height))
        .flex_row()
        .items_center()
        .gap(px(6.))
        .when(separator, |view| {
            view.border_b_1()
                .border_color(move || theme().foreground.opacity(0.06))
        })
        .child(
            text(label)
                .text_size(body_size)
                .when(label == "BIN 基地址", |text| text.font_family(mono)),
        )
}

fn valid_base(value: &Preferences) -> bool {
    value.get("base").trim().is_empty() || crate::model::number(value.get("base")).is_ok()
}

fn chip_picker(
    store: Store,
    draft: Signal<Preferences>,
    selected: Signal<Option<String>>,
    back: Signal<bool>,
    open: Signal<bool>,
    standalone: bool,
) -> View {
    let query = signal(String::new());
    let results = signal(Vec::<String>::new());
    let searching = signal(false);
    let error = signal(String::new());
    let revision = stored(0_u64);
    let initial = draft.with_untracked(|value| value.get("chip").to_owned());
    let candidates = store.read(|state| state.chips.clone());
    selected.set(Some(
        candidates
            .first()
            .cloned()
            .unwrap_or_else(|| initial.clone()),
    ));
    let initial = stored(initial);
    let candidates = stored(candidates);
    let displayed = computed(move || {
        if query.with(String::is_empty) {
            candidates.with(|candidates| {
                if candidates.is_empty() {
                    initial.with(|initial| {
                        if initial.is_empty() {
                            Vec::new()
                        } else {
                            vec![initial.clone()]
                        }
                    })
                } else {
                    candidates.clone()
                }
            })
        } else {
            results.get()
        }
    });
    let automatic =
        move || query.with(String::is_empty) && candidates.with(|values| values.is_empty());
    let names = computed(move || {
        let mut names = displayed.get();
        if automatic() {
            names.insert(0, String::new());
        }
        names
    });
    let source = store.read(|state| state.worker.core.clone());
    effect(move || {
        let text = query.get();
        let request = revision.with(|revision| {
            *revision += 1;
            *revision
        });
        results.set(Vec::new());
        error.set(String::new());
        if text.trim().is_empty() {
            searching.set(false);
            if text.is_empty() {
                selected.set(Some(
                    candidates
                        .with(|values| values.first().cloned())
                        .unwrap_or_else(|| initial.get()),
                ));
            } else {
                selected.set(None);
            }
            return;
        }
        selected.set(None);
        searching.set(true);
        let core = source.clone();
        spawn(async move {
            timer(Duration::from_millis(200)).await;
            if revision
                .try_with(|revision| *revision != request)
                .unwrap_or(true)
            {
                return;
            }
            let found = nsh_core::element::blocking(move || core.search_chips(text, 150)).await;
            if revision
                .try_with(|revision| *revision != request)
                .unwrap_or(true)
            {
                return;
            }
            match found {
                Ok(Ok(names)) => {
                    selected.set(names.first().cloned());
                    results.set(names);
                }
                Ok(Err(problem)) => error.set(format!("{problem:#}")),
                Err(problem) => error.set(problem.to_string()),
            }
            searching.set(false);
        });
    });
    let choose = move || {
        if searching.get_untracked() {
            return;
        }
        if let Some(chip) = selected.get_untracked() {
            draft.update(|draft| {
                draft.fields.insert("chip".into(), chip.clone());
            });
            if standalone {
                store.change(|state| state.set("chip", chip));
            } else {
                back.set(false);
            }
            if candidates.with(|candidates| !candidates.is_empty()) {
                connect_from_menu(store);
            }
            if standalone {
                open.set(false);
            }
        }
    };
    on_key_with(
        "enter",
        KeyOptions {
            when: nsh_core::keymap::When::NotEditing,
            ..Default::default()
        },
        move |_| choose(),
    );
    let list_size = signal(Size {
        width: px(452.),
        height: px(300.),
    });
    let focused = signal(false);
    let list_ref = element_ref();
    list_ref.on_focus_in(move || focused.set(true));
    list_ref.on_focus_out(move || focused.set(false));
    let data =
        computed(move || names.with(|names| DataTableRows::new(names.len()).texts("name", names)));
    let mut classes = DataTableClasses::default();
    classes.root = Some("border-0 bg-transparent rounded-none".into());
    classes.header = Some("hidden".into());
    classes.cell = Some("p-0".into());
    classes.row = Some("bg-transparent border-0".into());
    classes.row_selected = Some("bg-transparent".into());
    view()
        .w(px(500.))
        .h(px(480.))
        .p_6()
        .gap(px(14.))
        .child(
            view()
                .gap(px(3.))
                .child(
                    text(move || {
                        if candidates.with(|values| values.is_empty()) {
                            "选择芯片"
                        } else {
                            "选择识别到的目标"
                        }
                    })
                    .text_size(body_size)
                    .font_weight(FontWeight::BOLD),
                )
                .child(caption(move || {
                    if candidates.with(|values| values.is_empty()) {
                        "搜索 probe-rs 支持的芯片型号。"
                    } else {
                        "目标无法唯一识别，请从候选型号中选择。"
                    }
                })),
        )
        .child(
            view()
                .h(px(24.))
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    view()
                        .flex_1()
                        .h(px(24.))
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .px(px(7.))
                        .rounded_full()
                        .border_1()
                        .border_color(hairline)
                        .bg(move || {
                            theme()
                                .color("record-search-well")
                                .unwrap_or(theme().background)
                        })
                        .focus_within(|style| {
                            style.outline(px(3.5)).outline_color(move || {
                                theme().color("record-search-ring").unwrap_or(theme().ring)
                            })
                        })
                        .child(
                            icons::icon("search")
                                .size(px(14.))
                                .text_color(move || theme().muted_foreground),
                        )
                        .child(
                            nsh_base::input()
                                .label("搜索芯片型号")
                                .placeholder("搜索芯片型号，例如 STM32F407")
                                .value(query)
                                .on_change(move |value: &str| query.set(value.into()))
                                .on_submit(move |_: &str| choose())
                                .flex_1()
                                .min_w_0()
                                .h(px(20.))
                                .text_size(body_size)
                                .caret_color(|| color(theme().primary))
                                .placeholder_color(|| color(theme().muted_foreground)),
                        )
                        .child(when(
                            move || !query.with(String::is_empty),
                            move || {
                                nsh_base::button()
                                    .size(px(14.))
                                    .accessibility_label("清除搜索内容")
                                    .on_press(move || query.set(String::new()))
                                    .rounded_full()
                                    .bg(move || theme().muted_foreground)
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        icons::icon("close")
                                            .size(px(9.))
                                            .text_color(move || theme().background),
                                    )
                            },
                        )),
                )
                .child(when(searching, || caption("…"))),
        )
        .child(when(
            move || !error.with(String::is_empty),
            move || {
                text(move || error.get())
                    .text_size(caption_size)
                    .text_color(move || theme().destructive)
            },
        ))
        .child(
            view()
                .relative()
                .flex_1()
                .min_h_0()
                .border_1()
                .border_color(hairline)
                .overflow_hidden()
                .on_layout(move |bounds: &Bounds<Points>| list_size.set(bounds.size))
                .child(
                    view()
                        .absolute()
                        .inset_0()
                        .pointer_events(nsh_core::PointerEvents::None)
                        .accessibility_elements_hidden(true)
                        .children((0..20).map(move |index| {
                            view().h(px(24.)).flex_shrink_0().bg(move || {
                                if index % 2 == 1 {
                                    theme().color("table-stripe").unwrap_or(theme().muted)
                                } else {
                                    theme().background
                                }
                            })
                        })),
                )
                .child(
                    data_table()
                        .element_ref(&list_ref)
                        .columns(move || {
                            let mut column = DataTableColumn::default();
                            column.key = "name".into();
                            column.title = Some(SharedString::default());
                            column.width = Some(px((list_size.get().width.0 - 2.).max(1.)));
                            column.min_width = Some(px(1.));
                            vec![column]
                        })
                        .data(data)
                        .header_height(px(1.))
                        .row_height(px(24.))
                        .row_header(false)
                        .row_selectable(true)
                        .column_selectable(false)
                        .cell_selectable(false)
                        .loop_selection(false)
                        .resizable_columns(false)
                        .movable_columns(false)
                        .horizontal_scrollbar(false)
                        .font_size(body_size)
                        .classes(classes)
                        .accessibility_label("芯片型号")
                        .selection(move || {
                            selected.with(|selection| {
                                names.with(|names| {
                                    selection
                                        .as_ref()
                                        .and_then(|selected| {
                                            names.iter().position(|name| name == selected)
                                        })
                                        .map(|row| {
                                            let mut selection = DataTableSelection::default();
                                            selection.row = Some(row as u32);
                                            selection
                                        })
                                })
                            })
                        })
                        .on_selection_change(move |selection: &Option<DataTableSelection>| {
                            selected.set(
                                selection
                                    .as_ref()
                                    .and_then(|selection| selection.row)
                                    .and_then(|row| {
                                        names.with_untracked(|names| {
                                            names.get(row as usize).cloned()
                                        })
                                    }),
                            )
                        })
                        .on_row_press(move |press: &DataTablePress| {
                            selected
                                .set(names.with_untracked(|names| {
                                    names.get(press.row as usize).cloned()
                                }));
                        })
                        .render_cell(move |cell: &DataTableCell| {
                            let name = names
                                .with_untracked(|names| names.get(cell.row as usize).cloned())
                                .unwrap_or_default();
                            let automatic = name.is_empty();
                            let row_name = name.clone();
                            let is_selected = computed(move || {
                                selected
                                    .with(|selected| selected.as_deref() == Some(row_name.as_str()))
                            });
                            view()
                                .h(px(24.))
                                .px(px(8.))
                                .flex_row()
                                .items_center()
                                .gap(px(6.))
                                .bg(move || {
                                    if is_selected.get() {
                                        if focused.get() {
                                            theme()
                                                .color("selection-active")
                                                .unwrap_or(theme().primary)
                                        } else {
                                            theme()
                                                .color("selection-inactive")
                                                .unwrap_or(theme().secondary)
                                        }
                                    } else {
                                        theme().background.opacity(0.)
                                    }
                                })
                                .child(
                                    icons::icon(if automatic { "auto-detect" } else { "memory" })
                                        .size(px(14.))
                                        .text_color(move || {
                                            if is_selected.get() && focused.get() {
                                                theme().primary_foreground
                                            } else {
                                                theme().primary
                                            }
                                        }),
                                )
                                .child(
                                    text(if automatic {
                                        "自动识别".into()
                                    } else {
                                        name
                                    })
                                    .text_size(body_size)
                                    .text_color(move || {
                                        if is_selected.get() && focused.get() {
                                            theme().primary_foreground
                                        } else {
                                            theme().foreground
                                        }
                                    })
                                    .when(!automatic, |text| text.font_family(mono)),
                                )
                        })
                        .h(move || px(list_size.get().height.0 + 1.))
                        .mt(px(-1.)),
                )
                .child(when(
                    move || {
                        !query.with(String::is_empty)
                            && displayed.with(Vec::is_empty)
                            && !searching.get()
                            && error.with(String::is_empty)
                    },
                    move || {
                        view()
                            .absolute()
                            .inset_0()
                            .items_center()
                            .justify_center()
                            .gap(px(8.))
                            .px_6()
                            .translate_y(px(-10.))
                            .pointer_events(nsh_core::PointerEvents::None)
                            .child(
                                icons::icon("search")
                                    .size(px(44.))
                                    .mb(px(12.))
                                    .text_color(move || theme().muted_foreground.opacity(0.6)),
                            )
                            .child(
                                text(move || format!("未找到“{}”的相关结果", query.get()))
                                    .text_size(px(15.))
                                    .max_w(px(320.))
                                    .line_height(nsh_core::LineHeight::Multiple(4. / 3.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_center()
                                    .text_color(move || theme().muted_foreground),
                            )
                            .child(
                                text("检查拼写或尝试新搜索词。")
                                    .text_size(px(15.))
                                    .text_color(move || theme().muted_foreground),
                            )
                    },
                )),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(when(
                    move || !displayed.with(Vec::is_empty),
                    move || {
                        text(move || format!("{} 个目标", displayed.with(Vec::len)))
                            .text_size(small_size)
                            .text_color(move || theme().muted_foreground)
                    },
                ))
                .child(view().flex_1())
                .child(action(
                    if standalone { "取消" } else { "返回" },
                    Variant::Default,
                    || false,
                    move || {
                        if standalone {
                            open.set(false)
                        } else {
                            back.set(false)
                        }
                    },
                ))
                .child(action(
                    move || {
                        if candidates.with(|values| values.is_empty()) {
                            "选择"
                        } else {
                            "选择并连接"
                        }
                    },
                    Variant::DefaultAction,
                    move || searching.get() || selected.with(Option::is_none),
                    choose,
                )),
        )
}

fn valid_speed(value: &Preferences) -> bool {
    let speed = value.get("speed").trim();
    !speed.is_empty()
        && speed.bytes().all(|value| value.is_ascii_digit())
        && speed.parse::<u32>().is_ok_and(|speed| speed > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_speed_matches_native_decimal_input() {
        let mut draft = Preferences::default();
        for (value, expected) in [
            (" 4000 ", true),
            ("4294967295", true),
            ("4294967296", false),
            ("0", false),
            ("0x1000", false),
            ("+4000", false),
            ("4_000", false),
            ("", false),
        ] {
            draft.fields.insert("speed".into(), value.into());
            assert_eq!(valid_speed(&draft), expected, "{value}");
        }
    }

    #[test]
    fn blank_or_valid_bin_addresses_preserve_native_validation() {
        let mut draft = Preferences::default();
        for (value, expected) in [
            ("", true),
            ("  ", true),
            ("0x08000000", true),
            ("134217728", true),
            ("0x", false),
            ("-1", false),
            ("0800ZZ", false),
        ] {
            draft.fields.insert("base".into(), value.into());
            assert_eq!(valid_base(&draft), expected, "{value}");
        }
    }
}
