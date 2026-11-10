use super::*;
use gpui_kit::component::input::Editor;
impl Workbench {
    fn export_records(&mut self, format: &'static str, cx: &mut Context<Self>) {
        if let Some(session) = self.sessions.iter().find(|v| Some(v.id) == self.active) {
            match chipdock_gpui::presentation::records(
                &session.frames.iter().collect::<Vec<_>>(),
                format,
                self.snapshot(cx).get("display"),
            ) {
                Ok(bytes) => self.export_bytes(&format!("ChipDock-records.{format}"), bytes, cx),
                Err(e) => self.report_error(e.to_string(), cx),
            }
        }
    }
    pub(super) fn terminal_page(&self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let total = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .map_or(0, |s| s.frames.len());
        let mut terminal = div().size_full().flex().flex_col();
        if self.expanded.contains("terminal-text") {
            terminal = terminal.child(
                Editor::new(&self.terminal)
                    .font_family(cx.theme().mono_font_family.clone())
                    .flex_1()
                    .min_h_0()
                    .readonly(true),
            );
        } else {
            terminal = terminal.child(self.terminal_table(cx));
        }
        let shown = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .map_or(0, |_| self.terminal_rows.len());
        let tint = cx.theme().primary;
        let plain = cx.theme().foreground.opacity(0.75);
        let action = |id: &'static str, icon: &'static str, on: bool| {
            Button::new(id)
                .ghost()
                .size(px(22.))
                .p_0()
                .text_color(if on { tint } else { plain })
                .child(icons::icon(icon).size(px(15.)).flex_shrink_0())
        };
        let rule = cx.theme().foreground.opacity(0.12);
        let divider = move || div().mx(px(4.)).w(px(1.)).h(px(14.)).bg(rule);
        let composer_shown = !self.expanded.contains("hide-composer");
        terminal = terminal.child(
            div()
                .flex_shrink_0()
                .px(px(12.))
                .py(px(5.))
                .flex()
                .items_center()
                .gap(px(10.))
                .border_t_1()
                .border_color(cx.theme().foreground.opacity(0.1))
                .child(div().w(px(82.)).child(self.popup_sized(
                    "display",
                    ["utf-8", "hex", "ascii", "gbk", "latin-1"]
                        .into_iter()
                        .map(|v| (v.to_string(), super::components::option_label(v).to_string()))
                        .collect(),
                    true,
                    cx,
                )))
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(cx.theme().muted_foreground)
                        .child(if shown == total {
                            format!("{total} 条")
                        } else {
                            format!("{shown} / {total} 条")
                        }),
                )
                .child(div().flex_1().min_w(px(12.)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            action("auto-scroll", "arrow-down", self.preferences.flag("auto_scroll"))
                                .tooltip(if self.preferences.flag("auto_scroll") {
                                    "关闭自动滚动"
                                } else {
                                    "滚动到最新数据"
                                })
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.preferences.flags.insert(
                                        "auto_scroll".into(),
                                        !s.preferences.flag("auto_scroll"),
                                    );
                                    cx.notify();
                                })),
                        )
                        .child(
                            action("pause-display", "pause", self.paused)
                                .tooltip(if self.paused { "继续显示" } else { "暂停显示" })
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.paused = !s.paused;
                                    if !s.paused {
                                        s.render_terminal(w, cx);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(divider())
                        .child(
                            action("display-options", "settings", false)
                                .tooltip("显示选项")
                                .dropdown_menu({
                                    let owner = cx.entity().downgrade();
                                    let flags = self.preferences.flags.clone();
                                    move |mut m, _, _| {
                                        let toggle = |m: gpui_kit::component::menu::PopupMenu, key: &'static str, label: &'static str| {
                                            let checked = flags.get(key).copied().unwrap_or(false);
                                            let owner = owner.clone();
                                            m.item(PopupMenuItem::new(label).checked(checked).on_click(
                                                move |_, w, cx| {
                                                    let _ = owner.update(cx, |s, cx| {
                                                        s.preferences.flags.insert(key.into(), !checked);
                                                        s.render_terminal(w, cx);
                                                        cx.notify();
                                                    });
                                                },
                                            ))
                                        };
                                        m = toggle(m, "mixed", "文本与 HEX 混显");
                                        m = toggle(m, "timestamps", "显示时间戳");
                                        let colors = owner.clone();
                                        m = m.item(PopupMenuItem::new("着色规则…").on_click(
                                            move |_, w, cx| {
                                                let _ = colors.update(cx, |s, cx| s.open_colors(w, cx));
                                            },
                                        ));
                                        let reset = owner.clone();
                                        m = m.separator().item(PopupMenuItem::new("重置计数").on_click(
                                            move |_, _, cx| {
                                                let _ = reset.update(cx, |s, cx| {
                                                    s.serial_action("resetCounters", cx)
                                                });
                                            },
                                        ));
                                        m = toggle(m, "reset_on_clear", "清屏时重置计数");
                                        let text = owner.clone();
                                        m = m.separator().item(
                                            PopupMenuItem::new("切换可选中文本视图").on_click(
                                                move |_, _, cx| {
                                                    let _ = text.update(cx, |s, cx| {
                                                        if !s.expanded.remove("terminal-text") {
                                                            s.expanded.insert("terminal-text");
                                                        }
                                                        cx.notify();
                                                    });
                                                },
                                            ),
                                        );
                                        let owner = owner.clone();
                                        m.item(PopupMenuItem::new("复制终端").on_click(move |_, _, cx| {
                                            let _ = owner.update(cx, |s, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    s.terminal.read(cx).value().to_string(),
                                                ))
                                            });
                                        }))
                                    }
                                }),
                        )
                        .child(
                            action("records", "table", false)
                                .tooltip("记录表")
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.open_serial_tool("records", "记录表", w, cx)
                                })),
                        )
                        .child(
                            action("terminal-export", "export", false)
                                .tooltip("导出记录")
                                .disabled(total == 0)
                                .dropdown_menu({
                                    let owner = cx.entity().downgrade();
                                    move |mut m, _, _| {
                                        for format in ["json", "txt", "csv", "bin"] {
                                            let owner = owner.clone();
                                            m = m.item(
                                                PopupMenuItem::new(format.to_uppercase()).on_click(
                                                    move |_, _, cx| {
                                                        let _ = owner.update(cx, |s, cx| {
                                                            s.export_records(format, cx)
                                                        });
                                                    },
                                                ),
                                            );
                                        }
                                        m
                                    }
                                }),
                        )
                        .child(
                            action("clear", "trash", false)
                                .tooltip(if cfg!(target_os = "macos") {
                                    "清空终端（⌘L）"
                                } else {
                                    "清空终端（Ctrl+L）"
                                })
                                .disabled(total == 0)
                                .on_click(cx.listener(|s, _, w, cx| s.clear_terminal(w, cx))),
                        )
                        .child(divider())
                        .child(
                            action("composer-toggle", "composer", composer_shown)
                                .tooltip(if composer_shown { "收起发送区" } else { "展开发送区" })
                                .on_click(cx.listener(|s, _, _, cx| {
                                    if !s.expanded.remove("hide-composer") {
                                        s.expanded.insert("hide-composer");
                                    }
                                    cx.notify();
                                })),
                        ),
                ),
        );
        if self.expanded.contains("hide-composer") {
            return terminal.into_any_element();
        }
        v_resizable("terminal-composer")
            .child(
                resizable_panel()
                    .size_range(px(180.)..px(1800.))
                    .child(terminal),
            )
            .child(
                resizable_panel()
                    .size(px(190.))
                    .size_range(px(164.)..px(320.))
                    .child(self.composer(w, cx)),
            )
            .into_any_element()
    }
}
