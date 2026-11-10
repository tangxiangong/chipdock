use super::*;
use gpui_kit::component::input::Editor;
impl Workbench {
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
        terminal = terminal.child(
            div()
                .h(px(32.))
                .flex_shrink_0()
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(div().w(px(82.)).child(self.compact_choice(
                    "display",
                    &["utf-8", "hex", "ascii", "gbk", "latin-1"],
                    cx,
                )))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{total} 条")),
                )
                .child(div().flex_1())
                .child(
                    native_button("auto-scroll")
                        .small()
                        .ghost()
                        .icon(icons::icon("arrow-down"))
                        .tooltip("自动滚动")
                        .selected(self.preferences.flag("auto_scroll"))
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.preferences
                                .flags
                                .insert("auto_scroll".into(), !s.preferences.flag("auto_scroll"));
                            cx.notify();
                        })),
                )
                .child(
                    native_button("pause-display")
                        .small()
                        .ghost()
                        .icon(icons::icon("pause"))
                        .tooltip("暂停显示")
                        .selected(self.paused)
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.paused = !s.paused;
                            if !s.paused {
                                s.render_terminal(w, cx);
                            }
                            cx.notify();
                        })),
                )
                .child(div().w(px(1.)).h(px(14.)).bg(cx.theme().border))
                .child(
                    native_button("display-options")
                        .small()
                        .ghost()
                        .icon(icons::icon("settings"))
                        .tooltip("显示选项")
                        .dropdown_menu({
                            let owner = cx.entity().downgrade();
                            let flags = self.preferences.flags.clone();
                            move |mut m, _, _| {
                                for (key, label) in [
                                    ("mixed", "文本与 HEX 混显"),
                                    ("timestamps", "显示时间戳"),
                                    ("reset_on_clear", "清屏时重置计数"),
                                ] {
                                    let checked = flags.get(key).copied().unwrap_or(false);
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
                                let colors = owner.clone();
                                m = m.item(
                                    PopupMenuItem::new("着色规则…")
                                        .icon(icons::icon("palette"))
                                        .on_click(move |_, w, cx| {
                                            let _ = colors.update(cx, |s, cx| s.open_colors(w, cx));
                                        }),
                                );
                                let reset = owner.clone();
                                m = m.item(PopupMenuItem::new("重置计数").on_click(
                                    move |_, _, cx| {
                                        let _ = reset.update(cx, |s, cx| {
                                            s.serial_action("resetCounters", cx)
                                        });
                                    },
                                ));
                                let text = owner.clone();
                                m = m.item(PopupMenuItem::new("切换可选中文本视图").on_click(
                                    move |_, _, cx| {
                                        let _ = text.update(cx, |s, cx| {
                                            if !s.expanded.remove("terminal-text") {
                                                s.expanded.insert("terminal-text");
                                            }
                                            cx.notify();
                                        });
                                    },
                                ));
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
                    native_button("records")
                        .small()
                        .ghost()
                        .icon(icons::icon("table"))
                        .tooltip("记录表")
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.open_serial_tool("records", "记录表", w, cx)
                        })),
                )
                .child(
                    native_button("terminal-export")
                        .small()
                        .ghost()
                        .icon(icons::icon("export"))
                        .tooltip("导出记录")
                        .disabled(total == 0)
                        .dropdown_menu({
                            let owner = cx.entity().downgrade();
                            move |mut m, _, _| {
                                for format in ["json", "txt", "csv", "bin"] {
                                    let owner = owner.clone();
                                    m = m.item(PopupMenuItem::new(format.to_uppercase()).on_click(
                                        move |_, _, cx| {
                                            let _ = owner.update(cx, |s, cx| {
                                                if let Some(session) = s
                                                    .sessions
                                                    .iter()
                                                    .find(|v| Some(v.id) == s.active)
                                                {
                                                    match chipdock_gpui::presentation::records(
                                                        &session.frames.iter().collect::<Vec<_>>(),
                                                        format,
                                                        s.snapshot(cx).get("display"),
                                                    ) {
                                                        Ok(bytes) => s.export_bytes(
                                                            &format!("ChipDock-records.{format}"),
                                                            bytes,
                                                            cx,
                                                        ),
                                                        Err(e) => s.report_error(e.to_string(), cx),
                                                    }
                                                }
                                            });
                                        },
                                    ));
                                }
                                m
                            }
                        }),
                )
                .child(
                    native_button("clear")
                        .small()
                        .ghost()
                        .icon(icons::icon("trash"))
                        .tooltip("清屏")
                        .disabled(total == 0)
                        .on_click(cx.listener(|s, _, w, cx| s.clear_terminal(w, cx))),
                )
                .child(div().w(px(1.)).h(px(14.)).bg(cx.theme().border))
                .child(
                    native_button("composer-toggle")
                        .small()
                        .ghost()
                        .icon(icons::icon("composer"))
                        .tooltip("发送区")
                        .selected(!self.expanded.contains("hide-composer"))
                        .on_click(cx.listener(|s, _, _, cx| {
                            if !s.expanded.remove("hide-composer") {
                                s.expanded.insert("hide-composer");
                            }
                            cx.notify();
                        })),
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
