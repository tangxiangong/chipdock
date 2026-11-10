use super::*;

impl Workbench {
    pub(super) fn toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let serial = self.page == "serial";
        let leading = if cx.platform_info().os == OperatingSystem::Mac {
            95.
        } else {
            80.
        };
        // AppKit centres the principal item but never lets it cover the title.
        let status_left =
            ((window.viewport_size().width - px(360.)) / 2. - px(leading)).max(px(388.));
        let dark = cx.theme().is_dark();
        let mut bar = gpui_kit::component::TitleBar::new()
            .h(px(52.))
            .pl(px(leading))
            .pr(px(9.))
            .bg(cx.theme().background)
            .border_b(px(0.5))
            .border_color(hsla(0., 0., if dark { 1. } else { 0. }, 0.11))
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .w_full()
                    .child(
                        glass(cx).w(px(221.)).justify_center().children(
                            [("flash", "固件烧录"), ("serial", "串口连接")]
                                .into_iter()
                                .map(|(page, label)| {
                                    let selected = self.page == page;
                                    Button::new(page)
                                        .ghost()
                                        .w(px(73.))
                                        .h(px(28.))
                                        .rounded_full()
                                        .accessibility_label(label)
                                        .toggled(selected)
                                        .when(selected, |b| {
                                            b.bg(rgb(if dark { 0x484848 } else { 0xe8e8e8 }))
                                        })
                                        .child(div().text_size(px(13.)).child(label))
                                        .on_click(cx.listener(move |s, _, _, cx| {
                                            s.confirmation = None;
                                            s.page = page;
                                            cx.notify();
                                        }))
                                }),
                        ),
                    )
                    .when(serial, |bar| {
                        let shown = !self.expanded.contains("hide-connection");
                        bar.child(
                            glass(cx)
                                .w(px(36.))
                                .justify_center()
                                .when(shown, |g| g.bg(cx.theme().primary).border_0())
                                .child(
                                    Button::new("sidebar-toggle")
                                        .ghost()
                                        .size(px(34.))
                                        .p_0()
                                        .rounded_full()
                                        .text_color(if shown {
                                            gpui_kit::white()
                                        } else {
                                            cx.theme().foreground.opacity(0.85)
                                        })
                                        .toggled(shown)
                                        .tooltip(if shown {
                                            "收起连接区"
                                        } else {
                                            "展开连接区"
                                        })
                                        .child(icons::icon("sidebar").size(px(18.)).flex_shrink_0())
                                        .on_click(cx.listener(|s, _, _, cx| {
                                            if !s.expanded.remove("hide-connection") {
                                                s.expanded.insert("hide-connection");
                                            }
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground.opacity(0.7))
                            .flex_shrink_0()
                            .child(match self.page {
                                "serial" => "串口连接",
                                "flash" => "固件烧录",
                                "profiles" => "配置与历史",
                                _ => "ChipDock",
                            }),
                    )
                    .child(div().flex_1())
                    .when(!serial, |bar| {
                        bar.child(
                            glass(cx)
                                .absolute()
                                .left(status_left)
                                .w(px(360.))
                                .px_3()
                                .justify_center()
                                .gap(px(8.))
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .overflow_hidden()
                                .children(self.activity(cx)),
                        )
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
    /// Xcode-style activity: live progress while a probe job runs, otherwise
    /// the latest status.
    fn activity(&self, cx: &App) -> Vec<AnyElement> {
        let busy = self.busy_lanes.contains(&Lane::Probe);
        if busy && let Some(progress) = self.progress {
            let message = without_percentage(&self.message);
            return vec![
                div()
                    .min_w_0()
                    .truncate()
                    .child(message.to_owned())
                    .into_any_element(),
                div()
                    .w(px(90.))
                    .flex_shrink_0()
                    .child(
                        gpui_kit::component::progress::Progress::new("activity-progress")
                            .value(progress * 100.),
                    )
                    .into_any_element(),
                div()
                    .min_w(px(34.))
                    .flex_shrink_0()
                    .text_right()
                    .child(format!("{:.0}%", progress * 100.))
                    .into_any_element(),
            ];
        }
        let completed = self.completed && !busy;
        let mut items = vec![];
        if busy {
            items.push(
                gpui_kit::component::spinner::Spinner::new()
                    .small()
                    .into_any_element(),
            );
        }
        if completed {
            items.push(
                icons::icon("check")
                    .size(px(14.))
                    .text_color(rgb(0x34c759))
                    .into_any_element(),
            );
        }
        items.push(
            div()
                .min_w_0()
                .truncate()
                .when(completed, |d| {
                    d.font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                })
                .child(self.message.clone())
                .into_any_element(),
        );
        items
    }
    fn serial_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let dark = cx.theme().is_dark();
        let direction = self.fields["direction"].read(cx).value().to_string();
        let searchless =
            self.terminal_rows.is_empty() || self.fields["filter"].read(cx).value().is_empty();
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                glass(cx).px(px(4.)).children(
                    [("all", "全部"), ("receive", "接收"), ("send", "发送")]
                        .into_iter()
                        .map(|(key, label)| {
                            let selected = direction == key;
                            Button::new(key)
                                .ghost()
                                .h(px(28.))
                                .px(px(12.))
                                .rounded_full()
                                .toggled(selected)
                                .when(selected, |b| {
                                    b.bg(rgb(if dark { 0x484848 } else { 0xe8e8e8 }))
                                })
                                .child(div().text_size(px(13.)).child(label))
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    s.set("direction", key, w, cx);
                                    s.render_terminal(w, cx);
                                }))
                        }),
                ),
            )
            .child(
                glass(cx).child(
                    Button::new("search-options")
                        .ghost()
                        .h(px(34.))
                        .px(px(9.))
                        .rounded_full()
                        .text_color(cx.theme().foreground.opacity(0.85))
                        .tooltip("搜索选项")
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(3.))
                                .child(icons::icon("filter").size(px(18.)).flex_shrink_0())
                                .child(icons::icon("chevron-bold").size(px(9.)).flex_shrink_0()),
                        )
                        .dropdown_menu({
                            let owner = cx.entity().downgrade();
                            let regex = self.preferences.flag("search_regex");
                            let case = self.preferences.flag("search_case");
                            move |mut m, _, _| {
                                for (key, label, checked) in [
                                    ("search_case", "区分大小写", case),
                                    ("search_regex", "正则表达式", regex),
                                ] {
                                    let owner = owner.clone();
                                    m = m.item(
                                        PopupMenuItem::new(label).checked(checked).on_click(
                                            move |_, w, cx| {
                                                let _ = owner.update(cx, |s, cx| {
                                                    s.preferences
                                                        .flags
                                                        .insert(key.into(), !checked);
                                                    s.render_terminal(w, cx);
                                                    cx.notify();
                                                });
                                            },
                                        ),
                                    );
                                }
                                m
                            }
                        }),
                ),
            )
            .child(
                glass(cx)
                    .w(px(254.))
                    .pl(px(12.))
                    .pr(px(4.))
                    .gap(px(4.))
                    .child(
                        icons::icon("search")
                            .size(px(16.))
                            .flex_shrink_0()
                            .text_color(cx.theme().foreground.opacity(0.7)),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.fields["filter"])
                                .appearance(false)
                                .text_size(px(13.))
                                .aria_label("搜索数据"),
                        ),
                    )
                    .children(
                        [(false, "up", "上一条命中"), (true, "down", "下一条命中")]
                            .into_iter()
                            .map(|(next, icon, label)| {
                                Button::new(icon)
                                    .ghost()
                                    .size(px(28.))
                                    .p_0()
                                    .rounded_full()
                                    .text_color(cx.theme().foreground.opacity(0.85))
                                    .child(icons::icon(icon).size(px(16.)).flex_shrink_0())
                                    .tooltip(label)
                                    .disabled(searchless)
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        let count = s.terminal_rows.len();
                                        if count > 0 {
                                            let index = s.selected_record.and_then(|seq| {
                                                s.terminal_rows
                                                    .iter()
                                                    .position(|r| r.sequence == seq)
                                            });
                                            let next_index = match index {
                                                None if next => 0,
                                                None => count - 1,
                                                Some(i) if next => (i + 1) % count,
                                                Some(i) => (i + count - 1) % count,
                                            };
                                            s.selected_record =
                                                Some(s.terminal_rows[next_index].sequence);
                                            s.preferences.flags.insert("auto_scroll".into(), false);
                                            s.terminal_scroll
                                                .scroll_to_item(next_index, ScrollStrategy::Center);
                                            cx.notify();
                                        }
                                    }))
                            }),
                    ),
            )
            .into_any_element()
    }
    fn flash_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let mac = cx.platform_info().os == OperatingSystem::Mac;
        let errors = self.unseen_errors;
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                glass(cx).child(
                    Button::new("profiles-menu")
                        .ghost()
                        .h(px(34.))
                        .px(px(7.))
                        .rounded_full()
                        .text_color(cx.theme().foreground.opacity(0.85))
                        .accessibility_label("配置")
                        .tooltip("存储或载入配置")
                        .disabled(self.is_busy())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(3.))
                                .child(icons::icon("bookmark").size(px(19.)))
                                .child(icons::icon("chevron-bold").size(px(9.))),
                        )
                        .dropdown_menu({
                            let owner = cx.entity().downgrade();
                            let profiles = self.preferences.profiles.clone();
                            move |mut menu, w, cx| {
                                let save = owner.clone();
                                menu = menu.item(PopupMenuItem::new("存储当前配置…").on_click(
                                    move |_, w, cx| {
                                        let _ = save.update(cx, |s, cx| s.open_save_profile(w, cx));
                                    },
                                ));
                                if profiles.is_empty() {
                                    return menu;
                                }
                                menu = menu.separator();
                                for (name, p) in &profiles {
                                    let owner = owner.clone();
                                    let p = p.clone();
                                    menu = menu.item(PopupMenuItem::new(name.clone()).on_click(
                                        move |_, w, cx| {
                                            let _ = owner
                                                .update(cx, |s, cx| s.apply_profile(&p, w, cx));
                                        },
                                    ));
                                }
                                let names = profiles.keys().cloned().collect::<Vec<_>>();
                                let owner = owner.clone();
                                menu.separator().submenu(
                                    "删除配置",
                                    w,
                                    cx,
                                    move |mut menu, _, _| {
                                        for name in &names {
                                            let owner = owner.clone();
                                            let name = name.clone();
                                            menu = menu.item(
                                                PopupMenuItem::new(name.clone()).on_click(
                                                    move |_, w, cx| {
                                                        let _ = owner.update(cx, |s, cx| {
                                                            s.preferences.profiles.remove(&name);
                                                            s.save(w, cx);
                                                        });
                                                    },
                                                ),
                                            );
                                        }
                                        menu
                                    },
                                )
                            }
                        }),
                ),
            )
            .child(
                glass(cx)
                    .child(
                        div()
                            .relative()
                            .child(
                                toolbar_icon("logs-window", "logs", cx)
                                    .tooltip(match (errors, mac) {
                                        (0, true) => "日志（⌘J）".to_owned(),
                                        (0, false) => "日志（Ctrl+J）".to_owned(),
                                        (n, true) => format!("{n} 条新错误（⌘J）"),
                                        (n, false) => format!("{n} 条新错误（Ctrl+J）"),
                                    })
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        if let Err(e) = s.open_panel(auxiliary::Panel::Logs, cx) {
                                            s.report_error(e.to_string(), cx);
                                        }
                                    })),
                            )
                            .when(errors > 0, |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .top(px(2.))
                                        .right(px(2.))
                                        .min_w(px(16.))
                                        .h(px(16.))
                                        .px(px(4.))
                                        .rounded_full()
                                        .bg(rgb(0xff3b30))
                                        .text_color(gpui_kit::white())
                                        .text_size(px(10.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(errors.to_string()),
                                )
                            }),
                    )
                    .child(
                        toolbar_icon("memory-window", "memory", cx)
                            .tooltip(if mac {
                                "内存（⌥⌘M）"
                            } else {
                                "内存（Ctrl+Alt+M）"
                            })
                            .on_click(cx.listener(|s, _, _, cx| {
                                if let Err(e) = s.open_panel(auxiliary::Panel::Memory, cx) {
                                    s.report_error(e.to_string(), cx);
                                }
                            })),
                    ),
            )
            .into_any_element()
    }
}

/// Drops a trailing "（45%）" that duplicates the progress bar.
fn without_percentage(message: &str) -> &str {
    let trimmed = message.trim_end();
    let Some(body) = trimmed.strip_suffix(['）', ')']) else {
        return message;
    };
    let Some(open) = body.rfind(['（', '(']) else {
        return message;
    };
    let digits = &body[open..].trim_start_matches(['（', '(']);
    match digits.strip_suffix('%') {
        Some(n) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
            body[..open].trim_end()
        }
        _ => message,
    }
}

#[cfg(test)]
mod tests {
    use super::without_percentage;

    #[test]
    fn progress_message_drops_only_a_trailing_percentage() {
        assert_eq!(without_percentage("正在写入（45%）"), "正在写入");
        assert_eq!(without_percentage("Programming (7%)"), "Programming");
        assert_eq!(without_percentage("正在写入（扇区）"), "正在写入（扇区）");
        assert_eq!(without_percentage("准备就绪"), "准备就绪");
    }
}
