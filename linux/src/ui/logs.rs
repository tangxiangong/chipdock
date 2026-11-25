use super::*;

#[derive(Clone, Copy)]
struct LogWindow {
    entries: Memo<Vec<String>>,
    scroll: ElementRef,
    search: ElementRef,
}

pub(super) fn prepare(store: Store) {
    let entries = computed(move || {
        store.read(|s| {
            let query = s.preferences.get("log_search").to_lowercase();
            s.logs
                .iter()
                .filter(|line| {
                    line.to_lowercase().contains(&query)
                        && (!s.preferences.flag("errors_only") || line.contains("[错误]"))
                })
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let scroll_ref = element_ref();

    let search = element_ref();
    crate::ui::commands::watch_window(store, "logs", Some(search));
    on_key("mod+f", move |_| {
        crate::ui::commands::focus_search(search, &store.field("log_search"));
    });
    effect(move || {
        let count = entries.with(Vec::len);
        if count > 0 && store.flag("follow_logs") {
            scroll_ref.scroll_to_end(false);
        }
    });
    provide_context(LogWindow {
        entries,
        scroll: scroll_ref,
        search,
    });
}

#[derive(Debug, Clone, PartialEq, Event)]
pub enum LogEntriesEvent {
    /// Visible and error counts for the log window's subtitle.
    #[args(visible, errors)]
    Summary(u32, u32),
}

/// Selectable log output with follow-tail scrolling and the native empty states.
#[component]
pub fn LogEntries(cx: Scope<LogEntriesEvent>) -> impl IntoElement {
    let store = auxiliary_store();
    let LogWindow {
        entries,
        scroll: scroll_ref,
        ..
    } = use_context().expect("log window context");
    let previous = stored(0.0_f32);
    let counts = computed(move || {
        (
            entries.with(Vec::len) as u32,
            store.read(|s| s.logs.iter().filter(|line| line.contains("[错误]")).count() as u32),
        )
    });
    effect(move || {
        let (visible, errors) = counts.get();
        cx.emit(LogEntriesEvent::Summary(visible, errors));
    });
    when(move || entries.with(Vec::is_empty), move || empty(store)).otherwise(move || {
        scroll()
            .element_ref(&scroll_ref)
            .on_scroll(move |event: &nsh_core::element::ScrollEvent| {
                let old = previous.replace(event.offset.y.0);
                if event.offset.y.0 + 1. < old {
                    store.change(|s| s.flag("follow_logs", false));
                }
            })
            .flex_1()
            .child(
                view()
                    .px(px(21.))
                    .py(px(10.))
                    .font_family(mono)
                    .text_size(caption_size)
                    .line_height(px(15.))
                    .selectable(true)
                    .child(each(
                        computed(move || {
                            entries
                                .with(|rows| rows.iter().cloned().enumerate().collect::<Vec<_>>())
                        }),
                        |line: &(usize, String)| line.0,
                        move |line, _| {
                            text(move || line.with(|row| row.1.clone())).text_color(move || {
                                if line.with(|row| row.1.contains("[错误]")) {
                                    theme().destructive
                                } else {
                                    theme().foreground
                                }
                            })
                        },
                    )),
            )
    })
}

/// Native search, filters, clipboard, export and clear commands.
#[component]
pub fn LogActions() -> impl IntoElement {
    let store = auxiliary_store();
    let LogWindow { search, .. } = use_context().expect("log window context");
    glass()
        .pr(px(3.))
        .child(search_control(store, search))
        .child(view().w(px(3.5)))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .child(log_toggle(
                    store,
                    "errors_only",
                    "error-circle",
                    "仅显示错误",
                ))
                .child(log_toggle(
                    store,
                    "follow_logs",
                    "arrow-down-to-line",
                    "跟随最新日志",
                )),
        )
        .child(view().w(px(5.5)))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .child(log_tool(
                    "logs-copy-document",
                    "复制全部日志",
                    || false,
                    move || store.read(|s| s.logs.is_empty()),
                    move || {
                        drop(set_clipboard(store.read(|s| {
                            s.logs.iter().cloned().collect::<Vec<_>>().join("\n")
                        })));
                    },
                ))
                .child(log_tool(
                    "logs-export",
                    "导出日志…",
                    || false,
                    move || store.read(|s| s.logs.is_empty()),
                    move || {
                        store.export_as(
                            "ChipDock.log",
                            store
                                .read(|s| s.logs.iter().cloned().collect::<Vec<_>>().join("\n"))
                                .into_bytes(),
                            crate::ui::file_dialogs::ExportKind::DocumentText,
                        )
                    },
                ))
                .child(log_tool(
                    "logs-trash",
                    "清空日志",
                    || false,
                    move || store.read(|s| s.logs.is_empty()),
                    move || store.change(|s| s.logs.clear()),
                )),
        )
}

/// The native search field's frame includes its icon and clear button; keep the whole region at the measured native 180 pt.
fn search_control(store: Store, search: ElementRef) -> View {
    let focused = signal(false);
    let active = control_active();
    let focused_style = |s: nsh_core::element::StyleRefinement| {
        s.outline(px(3.5)).outline_color(move || {
            theme()
                .color("logs-search-ring")
                .unwrap_or(theme().primary.opacity(0.5))
        })
    };
    view()
        .w(px(180.))
        .h(px(36.))
        .pl(px(12.))
        .flex_row()
        .items_center()
        .gap(px(4.))
        .rounded_full()
        .focus_within(focused_style)
        .style_when(move || focused.get() && active.get(), focused_style)
        .child(
            icons::icon("search")
                .size(px(16.))
                .text_color(move || theme().foreground.opacity(0.7)),
        )
        .child(
            nsh_base::input()
                .value(move || store.field("log_search"))
                .on_change(move |value: &str| store.change(|s| s.set("log_search", value.into())))
                .on_focus(move || focused.set(true))
                .on_blur(move || focused.set(false))
                .on_key_press(move |key: &str| {
                    if key == "Escape" {
                        store.change(|s| s.set("log_search", String::new()));
                        // The hosted input blurs after reporting Escape; restore search focus next frame.
                        spawn(async move {
                            let _ = nsh_core::element::next_frame().await;
                            search.focus();
                        });
                    }
                })
                .element_ref(&search)
                .label("筛选日志")
                .caret_color(|| color(theme().primary))
                .placeholder("筛选日志")
                .flex_1()
                .min_w_0()
                .text_size(body_size),
        )
        .child(when(
            move || !store.field("log_search").is_empty(),
            move || {
                // NSSearchField's cancel cell has a 36 pt hit region with a 13 pt circular glyph.
                nsh_base::button()
                    .on_press(move || {
                        store.change(|s| s.set("log_search", String::new()));
                        search.focus();
                    })
                    .accessibility_label("清除日志筛选")
                    .size(px(36.))
                    .p_0()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .hover(|s| s.opacity(0.8))
                    .active(|s| s.opacity(0.65))
                    .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
                    .child(
                        view()
                            .size(px(13.))
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(move || theme().muted_foreground)
                            .text_color(move || theme().background)
                            .child(icons::icon("close").size(px(9.))),
                    )
            },
        ))
}

fn empty(store: Store) -> View {
    let state = computed(move || {
        store.read(|s| {
            if s.logs.is_empty() {
                0
            } else if s.preferences.get("log_search").is_empty() {
                1
            } else {
                2
            }
        })
    });
    super::empty_view_with_layout(
        move || match state.get() {
            0 => "logs-empty",
            1 => "logs-no-errors",
            _ => "logs-search-empty",
        },
        move || match state.get() {
            0 => "暂无日志".to_owned(),
            1 => "没有错误".to_owned(),
            _ => format!("未找到“{}”的相关结果", store.field("log_search")),
        },
        move || match state.get() {
            0 => "连接、烧录和读取内存的过程会记录在这里。",
            1 => "",
            _ => "检查拼写或尝试新搜索词。",
        },
        move || match state.get() {
            0 => super::EmptyLayout::Standard,
            1 => super::EmptyLayout::LabelOnly,
            _ => super::EmptyLayout::Search,
        },
    )
}
fn log_toggle(
    store: Store,
    key: &'static str,
    name: &'static str,
    label: &'static str,
) -> impl IntoElement {
    nsh_base::checkbox()
        .checked(move || store.flag(key))
        .on_change(move |checked: &bool| store.change(|s| s.flag(key, *checked)))
        .indicator_class("hidden")
        .accessibility_label(label)
        .size(px(30.))
        .p_0()
        .rounded_full()
        .items_center()
        .justify_center()
        .text_color(move || theme().foreground)
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().muted))
        .checked_style(|s| {
            s.bg(move || {
                theme()
                    .color("logs-control-selected")
                    .unwrap_or(theme().primary)
            })
            .text_color(move || theme().primary_foreground)
        })
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .child(icons::icon(name).size(px(20.75)))
}

fn log_tool(
    name: &'static str,
    label: &'static str,
    selected: impl Fn() -> bool + 'static,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    nsh_base::button()
        .on_press(press)
        .selected(selected)
        .disabled(disabled)
        .accessibility_label(label)
        .size(px(30.))
        .p_0()
        .rounded_full()
        .items_center()
        .justify_center()
        .text_color(move || theme().foreground)
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().muted))
        .selected_style(|s| {
            s.bg(move || theme().primary)
                .text_color(move || theme().primary_foreground)
        })
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .disabled_style(|s| s.opacity(0.30))
        .child(icons::icon(name).size(px(match name {
            "logs-copy-document" => 20.75,
            "logs-export" | "logs-trash" => 21.75,
            _ => 17.,
        })))
}

pub(super) fn register(registry: &mut Registry) {
    registry.register::<LogEntries>().register::<LogActions>();
}
