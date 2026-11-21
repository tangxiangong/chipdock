use super::*;

/// Mode selection linked to menu commands and dropped firmware.
#[component]
pub fn WorkbenchModes() -> impl IntoElement {
    let (_, mode) = workbench_context();
    let active = control_active();
    glass().w(px(221.)).mr(px(-4.)).justify_center().child(
        nsh_base::radio_group()
            .flex_row()
            .items_center()
            .gap_1()
            .children(
                [("flash", "固件烧录"), ("serial", "串口连接")]
                    .into_iter()
                    .map(move |(key, label)| {
                        nsh_base::radio()
                            .checked(move || mode.get() == key)
                            .on_change(move |checked: &bool| {
                                if *checked {
                                    mode.set(key);
                                }
                            })
                            .w(px(73.))
                            .h(px(28.))
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .checked_style(|style| {
                                style.bg(move || theme().foreground.opacity(0.09))
                            })
                            .hover(|s| s.bg(move || theme().accent))
                            .focus_visible(|s| {
                                s.outline(px(2.)).outline_color(move || theme().ring)
                            })
                            .child(text(label).text_size(body_size).text_color(move || {
                                if active.get() {
                                    theme().foreground
                                } else {
                                    theme()
                                        .color(if mode.get() == key {
                                            "toolbar-selection-inactive-foreground"
                                        } else {
                                            "toolbar-inactive-foreground"
                                        })
                                        .unwrap_or(theme().muted_foreground)
                                }
                            }))
                    }),
            ),
    )
}

/// Native sidebar toggle and window activation appearance.
#[component]
pub fn SidebarToggle() -> impl IntoElement {
    let (store, _) = workbench_context();
    let active = control_active();
    let window = nsh_core::element::desktop::current_window();
    let sidebar_active = computed(move || window.state().is_none_or(|state| state.focused));
    glass()
        .w(px(36.))
        .justify_center()
        .bg(move || {
            if store.sidebar.get() && sidebar_active.get() {
                theme().primary
            } else if store.sidebar.get() {
                theme()
                    .color("toolbar-selection-inactive")
                    .unwrap_or(theme().secondary)
            } else {
                theme().background.opacity(0.)
            }
        })
        .child(
            nsh_base::checkbox()
                .checked(move || store.sidebar.get())
                .indicator_style(|style| style.hidden())
                .on_change(move |checked: &bool| store.sidebar.set(*checked))
                .accessibility_label("连接区")
                .size(px(34.))
                .p_0()
                .items_center()
                .justify_center()
                .rounded_full()
                .text_color(move || {
                    if store.sidebar.get() && sidebar_active.get() {
                        theme().primary_foreground
                    } else if store.sidebar.get() {
                        theme()
                            .color("toolbar-sidebar-inactive")
                            .unwrap_or(theme().muted_foreground)
                    } else {
                        if active.get() {
                            theme().foreground.opacity(0.85)
                        } else {
                            theme()
                                .color("toolbar-sidebar-inactive")
                                .unwrap_or(theme().muted_foreground)
                        }
                    }
                })
                .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
                .child(icons::icon("sidebar").size(px(18.))),
        )
}
fn activity(store: Store) -> View {
    glass()
        .absolute()
        .top(px(8.))
        .left(move || px(((nsh_core::element::window().width.0 - 360.) / 2.).max(483.)))
        .w(px(360.))
        .px_3()
        .justify_center()
        .gap_2()
        .child(when(
            move || store.read(|s| s.progress.is_some()),
            move || {
                nsh_base::progress()
                    .value(move || store.read(|s| s.progress.unwrap_or(0.) * 100.))
                    .w(px(90.))
                    .accessibility_label("操作进度")
                    .track_class("h-1 bg-muted rounded-full")
                    .indicator_class("h-1 bg-primary rounded-full")
            },
        ))
        .child(
            text(move || store.read(|s| s.message.clone()))
                .text_size(caption_size)
                .text_color(move || theme().muted_foreground),
        )
}
fn flash_toolbar(store: Store) -> View {
    use nsh_core::element::desktop::MenuItem;
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            glass().child(native_menu_disabled(
                view()
                    .w(px(42.))
                    .h(px(34.))
                    .flex_row()
                    .gap_1()
                    .items_center()
                    .justify_center()
                    .child(
                        icons::icon("bookmark")
                            .size(px(20.))
                            .scale_x(0.95)
                            .translate_x(px(0.75))
                            .translate_y(px(1.5)),
                    )
                    .child(
                        icons::icon("toolbar-down")
                            .size(px(8.))
                            .translate_x(px(-1.))
                            .translate_y(px(1.)),
                    )
                    .label("配置")
                    .tooltip(text("存储或载入配置").text_size(caption_size)),
                move || !super::commands::probe_idle(store),
                move || {
                    let mut items = vec![
                        MenuItem::new("存储当前配置…")
                            .on_select(move || auxiliary::open(store, auxiliary::Panel::Profiles)),
                    ];
                    let profiles = store.read(|state| {
                        state
                            .preferences
                            .profiles
                            .keys()
                            .cloned()
                            .collect::<Vec<_>>()
                    });
                    if !profiles.is_empty() {
                        items.push(MenuItem::separator());
                        items.extend(profiles.iter().map(|name| {
                            let name = name.clone();
                            MenuItem::new(name.clone())
                                .on_select(move || store.change(|state| state.apply_profile(&name)))
                        }));
                        items.push(MenuItem::separator());
                        items.push(MenuItem::submenu(
                            "删除配置",
                            profiles.into_iter().map(|name| {
                                MenuItem::new(name.clone()).on_select(move || {
                                    super::commands::remove_profile(store, &name)
                                })
                            }),
                        ));
                    }
                    items
                },
            )),
        )
        .child(glass().child(logs_control(store)).child(toolbar_icon(
            "memory",
            "内存（⌥⌘M）",
            || false,
            || false,
            move || {
                if !store.modal.get_untracked() {
                    super::commands::toggle_panel(store, "memory", auxiliary::Panel::Memory);
                }
            },
        )))
}

fn logs_control(store: Store) -> View {
    let active = control_active();
    view()
        .relative()
        .w(px(38.))
        .h(px(34.))
        .role(AccessibilityRole::Button)
        .focusable(true)
        .label("日志")
        .accessibility_value(move || {
            let count = store.read(|state| state.unseen_errors);
            (count > 0).then(|| nsh_core::AccessibilityValue {
                text: Some(count.to_string().into()),
                ..Default::default()
            })
        })
        .tooltip(text(move || {
            let count = store.read(|state| state.unseen_errors);
            if count > 0 {
                format!("{count} 条新错误（⌘J）")
            } else {
                "日志（⌘J）".into()
            }
        }))
        .on_press(move |_: &()| {
            if !store.modal.get_untracked() {
                super::commands::toggle_panel(store, "logs", auxiliary::Panel::Logs);
            }
        })
        .items_center()
        .justify_center()
        .rounded_full()
        .text_color(move || {
            if active.get() {
                theme().foreground
            } else {
                theme().muted_foreground
            }
        })
        .hover(|style| style.bg(move || theme().accent))
        .active(|style| style.bg(move || theme().muted))
        .focus_visible(|style| style.outline(px(2.)).outline_color(move || theme().ring))
        .child(
            icons::icon("logs")
                .size(px(24.))
                .scale_x(33. / 34.)
                .scale_y(0.875)
                .translate_x(px(1.25)),
        )
        .child(when(
            move || store.read(|state| state.unseen_errors > 0),
            move || {
                view()
                    .absolute()
                    .right(px(1.))
                    .top(px(-1.))
                    .min_w(px(12.))
                    .h(px(12.))
                    .px(px(2.))
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(move || {
                        theme()
                            .color("notification-badge")
                            .unwrap_or(theme().destructive)
                    })
                    .text_color(move || theme().primary_foreground)
                    .accessibility_elements_hidden(true)
                    .child(
                        text(move || store.read(|state| state.unseen_errors.to_string()))
                            .text_size(px(8.))
                            .font_weight(FontWeight::SEMIBOLD),
                    )
            },
        ))
}

fn serial_toolbar(store: Store) -> View {
    let active = control_active();
    let narrow = computed(|| nsh_core::element::window().width.0 < 997.);
    view()
        .style_with(move |style| {
            if narrow.get() {
                style.absolute().left(px(528.)).right(px(9.)).top(px(8.))
            } else {
                style
            }
        })
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            glass().child(
                nsh_base::radio_group().flex_row().items_center().children(
                    [("all", "全部"), ("receive", "接收"), ("send", "发送")]
                        .into_iter()
                        .map(move |(key, label)| {
                            nsh_base::radio()
                                .checked(move || store.field("direction") == key)
                                .on_change(move |checked: &bool| {
                                    if *checked {
                                        store.change(|s| s.set("direction", key.into()));
                                    }
                                })
                                .w(px(49.))
                                .h(px(28.))
                                .p_0()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .checked_style(|s| s.bg(move || theme().foreground.opacity(0.09)))
                                .hover(|s| s.bg(move || theme().accent))
                                .focus_visible(|s| {
                                    s.outline(px(2.)).outline_color(move || theme().ring)
                                })
                                .child(text(label).text_size(body_size).text_color(move || {
                                    if active.get() {
                                        theme().foreground
                                    } else {
                                        theme()
                                            .color(if store.field("direction") == key {
                                                "toolbar-selection-inactive-foreground"
                                            } else {
                                                "toolbar-inactive-foreground"
                                            })
                                            .unwrap_or(theme().muted_foreground)
                                    }
                                }))
                        }),
                ),
            ),
        )
        .child(
            glass().child(native_menu(
                view()
                    .w(px(45.))
                    .h(px(36.))
                    .p_0()
                    .rounded_full()
                    .flex_row()
                    .gap(px(3.))
                    .items_center()
                    .justify_center()
                    .text_color(move || {
                        if active.get() {
                            theme().foreground
                        } else {
                            theme()
                                .color("toolbar-menu-inactive")
                                .unwrap_or(theme().muted_foreground)
                        }
                    })
                    .label("搜索选项")
                    .child(icons::icon("filter").size(px(18.)))
                    .child(icons::icon("down").size(px(9.))),
                move || {
                    vec![
                        native_toggle(store, "search_case", "区分大小写"),
                        native_toggle(store, "search_regex", "正则表达式"),
                    ]
                },
            )),
        )
        .child(dynamic(move || {
            if narrow.get() {
                view()
                    .flex_1()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .child(
                        glass()
                            .w(px(36.))
                            .justify_center()
                            .child(native_toolbar_menu(
                                view()
                                    .size(px(36.))
                                    .flex_row()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .gap(px(-5.))
                                    .text_color(move || {
                                        if active.get() {
                                            theme().foreground
                                        } else {
                                            theme()
                                                .color("toolbar-inactive-foreground")
                                                .unwrap_or(theme().muted_foreground)
                                        }
                                    })
                                    .label("更多工具栏项目")
                                    .child(icons::icon("right").size(px(11.)))
                                    .child(icons::icon("right").size(px(11.))),
                                move || {
                                    vec![
                                        nsh_core::element::desktop::MenuItem::new("上一条命中")
                                            .disabled(!can_navigate(store))
                                            .on_select(move || {
                                                super::serial::navigate(store, false)
                                            }),
                                        nsh_core::element::desktop::MenuItem::new("下一条命中")
                                            .disabled(!can_navigate(store))
                                            .on_select(move || {
                                                super::serial::navigate(store, true)
                                            }),
                                    ]
                                },
                            )),
                    )
                    .into_element()
            } else {
                glass()
                    .w(px(254.))
                    .ml(px(-2.))
                    .pl(px(2.))
                    .gap(px(0.))
                    .child(
                        search_field(
                            "搜索数据",
                            move || store.field("filter"),
                            move |value: &str| store.change(|s| s.set("filter", value.into())),
                            store.search,
                        )
                        .w(px(180.)),
                    )
                    .child(search_navigation(store, false))
                    .child(search_navigation(store, true))
                    .into_element()
            }
        }))
}

fn can_navigate(store: Store) -> bool {
    !store.field("filter").is_empty()
        && store.read(|state| {
            state.active_session().is_some_and(|session| {
                crate::presentation::filtered(session.records(), &state.preferences)
                    .is_ok_and(|rows| !rows.is_empty())
            })
        })
}

fn search_navigation(store: Store, next: bool) -> impl IntoElement {
    let disabled = move || {
        store.field("filter").is_empty()
            || store.read(|s| {
                s.active_session().is_none_or(|session| {
                    crate::presentation::filtered(session.records(), &s.preferences)
                        .map_or(true, |rows| rows.is_empty())
                })
            })
    };
    nsh_base::button()
        .disabled(disabled)
        .on_press(move || super::serial::navigate(store, next))
        .accessibility_label(if next {
            "下一条命中"
        } else {
            "上一条命中"
        })
        .w(px(36.))
        .h(px(28.))
        .p_0()
        .rounded_full()
        .items_center()
        .justify_center()
        .text_color(move || theme().foreground.opacity(0.85))
        .disabled_style(|view| view.opacity(0.35))
        .hover(|view| view.bg(move || theme().accent))
        .active(|view| view.bg(move || theme().muted))
        .focus_visible(|view| view.outline(px(2.)).outline_color(move || theme().ring))
        .child(icons::icon(if next { "down" } else { "up" }).size(px(16.)))
}

/// The ActivityIndicator control group bound to native workbench commands.
#[component]
pub fn ActivityIndicator() -> impl IntoElement {
    let (store, _) = workbench_context();
    activity(store)
}

/// The FirmwareActions control group bound to native workbench commands.
#[component]
pub fn FirmwareActions() -> impl IntoElement {
    let (store, _) = workbench_context();
    flash_toolbar(store)
}

/// The SerialActions control group bound to native workbench commands.
#[component]
pub fn SerialActions() -> impl IntoElement {
    let (store, _) = workbench_context();
    serial_toolbar(store)
}

pub(super) fn register(registry: &mut Registry) {
    registry
        .register::<WorkbenchModes>()
        .register::<SidebarToggle>()
        .register::<ActivityIndicator>()
        .register::<FirmwareActions>()
        .register::<SerialActions>();
}
