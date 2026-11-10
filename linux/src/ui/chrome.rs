use super::*;

impl Workbench {
    pub(super) fn toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let serial = self.page == "serial";
        let leading = if cx.platform_info().os == OperatingSystem::Mac {
            95.
        } else {
            80.
        };
        let status_inset =
            ((window.viewport_size().width - px(360.)) / 2. - px(leading + 296.)).max(px(0.));
        let mut bar = gpui_kit::component::TitleBar::new()
            .h(px(52.))
            .pl(px(leading))
            .pr_3()
            .bg(cx.theme().sidebar)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .w_full()
                    .child(
                        div()
                            .w(px(220.))
                            .flex_shrink_0()
                            .flex()
                            .justify_center()
                            .rounded_full()
                            .bg(control_bg(cx))
                            .p_1()
                            .children(
                                [("flash", "固件烧录"), ("serial", "串口连接")]
                                    .into_iter()
                                    .map(|(page, label)| {
                                        native_button(page)
                                            .small()
                                            .ghost()
                                            .w(px(74.))
                                            .h(px(28.))
                                            .rounded_full()
                                            .label(label)
                                            .selected(self.page == page)
                                            .on_click(cx.listener(move |s, _, _, cx| {
                                                s.confirmation = None;
                                                s.page = page;
                                                cx.notify();
                                            }))
                                    }),
                            ),
                    )
                    .child(if serial {
                        native_button("sidebar-toggle")
                            .small()
                            .ghost()
                            .icon(icons::icon("sidebar"))
                            .tooltip("显示或隐藏连接区")
                            .on_click(cx.listener(|s, _, _, cx| {
                                if !s.expanded.remove("hide-connection") {
                                    s.expanded.insert("hide-connection");
                                }
                                cx.notify();
                            }))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .flex_shrink_0()
                            .child(match self.page {
                                "serial" => "串口连接",
                                "flash" => "固件烧录",
                                "profiles" => "配置与历史",
                                _ => "ChipDock",
                            }),
                    )
                    .child(if serial {
                        div().flex_1().into_any_element()
                    } else {
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .pl(status_inset)
                            .child(
                                div()
                                    .w(px(360.))
                                    .max_w_full()
                                    .h(px(36.))
                                    .px_3()
                                    .rounded_full()
                                    .bg(control_bg(cx))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .overflow_hidden()
                                    .child(self.message.clone()),
                            )
                            .into_any_element()
                    })
                    .child(if serial {
                        self.serial_toolbar(cx)
                    } else {
                        self.flash_toolbar(cx)
                    }),
            );
        bar = bar.on_close_window(cx.listener(|s, _, w, cx| {
            if !s.is_busy() {
                s.save(w, cx);
                w.remove_window();
            }
        }));
        bar
    }
    fn serial_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex()
                    .rounded_full()
                    .bg(control_bg(cx))
                    .p_1()
                    .children(
                        [("all", "全部"), ("receive", "接收"), ("send", "发送")]
                            .into_iter()
                            .map(|(key, label)| {
                                native_button(key)
                                    .small()
                                    .ghost()
                                    .rounded_full()
                                    .label(label)
                                    .selected(
                                        self.fields["direction"].read(cx).value().as_ref() == key,
                                    )
                                    .on_click(cx.listener(move |s, _, w, cx| {
                                        s.set("direction", key, w, cx);
                                        s.render_terminal(w, cx);
                                    }))
                            }),
                    ),
            )
            .child(
                native_button("search-options")
                    .small()
                    .ghost()
                    .icon(icons::icon("filter"))
                    .tooltip("搜索选项")
                    .dropdown_menu({
                        let owner = cx.entity().downgrade();
                        let regex = self.preferences.flag("search_regex");
                        let case = self.preferences.flag("search_case");
                        move |mut m, _, _| {
                            for (key, label, checked) in [
                                ("search_regex", "正则表达式", regex),
                                ("search_case", "区分大小写", case),
                            ] {
                                let owner = owner.clone();
                                m = m.item(PopupMenuItem::new(label).checked(checked).on_click(
                                    move |_, w, cx| {
                                        let _ = owner.update(cx, |s, cx| {
                                            s.preferences.flags.insert(key.into(), !checked);
                                            s.render_terminal(w, cx);
                                            cx.notify();
                                        });
                                    },
                                ));
                            }
                            m
                        }
                    }),
            )
            .child(
                div().w(px(180.)).child(
                    Input::new(&self.fields["filter"])
                        .small()
                        .rounded_full()
                        .aria_label("搜索数据")
                        .prefix(icons::icon("search")),
                ),
            )
            .children(
                [(false, "up", "上一条命中"), (true, "down", "下一条命中")]
                    .into_iter()
                    .map(|(next, icon, label)| {
                        native_button(icon)
                            .small()
                            .ghost()
                            .icon(icons::icon(icon))
                            .tooltip(label)
                            .disabled(
                                self.terminal_rows.is_empty()
                                    || self.fields["filter"].read(cx).value().is_empty(),
                            )
                            .on_click(cx.listener(move |s, _, _, cx| {
                                let count = s.terminal_rows.len();
                                if count > 0 {
                                    let index = s.selected_record.and_then(|seq| {
                                        s.terminal_rows.iter().position(|r| r.sequence == seq)
                                    });
                                    let next_index = match index {
                                        None => 0,
                                        Some(i) if next => (i + 1) % count,
                                        Some(i) => (i + count - 1) % count,
                                    };
                                    s.selected_record = Some(s.terminal_rows[next_index].sequence);
                                    s.terminal_scroll
                                        .scroll_to_item(next_index, ScrollStrategy::Center);
                                    cx.notify();
                                }
                            }))
                    }),
            )
            .into_any_element()
    }
    fn flash_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                native_button("profiles-menu")
                    .small()
                    .ghost()
                    .icon(icons::icon("bookmark"))
                    .tooltip("配置")
                    .disabled(self.is_busy())
                    .dropdown_menu({
                        let owner = cx.entity().downgrade();
                        let profiles = self.preferences.profiles.clone();
                        move |mut menu, _, _| {
                            let save = owner.clone();
                            menu = menu.item(PopupMenuItem::new("存储当前配置…").on_click(
                                move |_, w, cx| {
                                    let _ = save.update(cx, |s, cx| s.open_save_profile(w, cx));
                                },
                            ));
                            for (name, p) in &profiles {
                                let owner = owner.clone();
                                let p = p.clone();
                                menu = menu.item(PopupMenuItem::new(name.clone()).on_click(
                                    move |_, w, cx| {
                                        let _ =
                                            owner.update(cx, |s, cx| s.apply_profile(&p, w, cx));
                                    },
                                ));
                            }
                            let owner = owner.clone();
                            menu.item(PopupMenuItem::new("管理配置与历史…").on_click(
                                move |_, _, cx| {
                                    let _ = owner.update(cx, |s, cx| {
                                        s.page = "profiles";
                                        cx.notify();
                                    });
                                },
                            ))
                        }
                    }),
            )
            .child(
                native_button("logs-window")
                    .small()
                    .ghost()
                    .icon(icons::icon("logs"))
                    .tooltip(if cx.platform_info().os == OperatingSystem::Mac {
                        "日志（⌘J）"
                    } else {
                        "日志（Ctrl+J）"
                    })
                    .on_click(cx.listener(|s, _, _, cx| {
                        if let Err(e) = s.open_panel(auxiliary::Panel::Logs, cx) {
                            s.report_error(e.to_string(), cx);
                        }
                    })),
            )
            .child(
                native_button("memory-window")
                    .small()
                    .ghost()
                    .icon(icons::icon("memory"))
                    .tooltip(if cx.platform_info().os == OperatingSystem::Mac {
                        "内存（⌥⌘M）"
                    } else {
                        "内存（Ctrl+Alt+M）"
                    })
                    .on_click(cx.listener(|s, _, _, cx| {
                        if let Err(e) = s.open_panel(auxiliary::Panel::Memory, cx) {
                            s.report_error(e.to_string(), cx);
                        }
                    })),
            )
            .into_any_element()
    }
}
