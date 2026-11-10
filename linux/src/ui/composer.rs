use super::*;
impl Workbench {
    pub(super) fn toggle_auto_send(&mut self, cx: &mut Context<Self>) {
        let repeating = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref())
            .is_some_and(|s| s.job_kind.as_deref() == Some("repeat"));
        if repeating {
            self.serial_action("cancelJob", cx);
        } else if self.serial_connected()
            && !self.is_busy()
            && !self.preferences.flag("command_mode")
        {
            self.serial_action("repeat", cx);
        }
    }

    pub(super) fn composer(&self, _w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let command = self.preferences.flag("command_mode");
        let mut header = div()
            .h(px(36.))
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div().flex().rounded_md().bg(control_bg(cx)).children(
                    [(false, "文本"), (true, "命令")]
                        .into_iter()
                        .map(|(mode, label)| {
                            native_button(if mode { "command-mode" } else { "normal-mode" })
                                .small()
                                .ghost()
                                .label(label)
                                .selected(command == mode)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.preferences.flags.insert("command_mode".into(), mode);
                                    cx.notify();
                                }))
                        }),
                ),
            );
        if !command {
            header = header
                .child(div().w(px(82.)).child(self.compact_choice(
                    "format",
                    &["utf-8", "hex", "ascii", "gbk", "latin-1"],
                    cx,
                )))
                .child(div().w(px(82.)).child(self.compact_choice(
                    "ending",
                    &["none", "lf", "cr", "crlf"],
                    cx,
                )))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(icons::icon("shield"))
                        .child(div().w(px(110.)).child(self.compact_choice(
                            "checksum",
                            &[
                                "none",
                                "sum8",
                                "xor8",
                                "crc16Modbus",
                                "crc16Ccitt",
                                "crc32",
                                "lrc",
                                "oddParity",
                                "evenParity",
                            ],
                            cx,
                        ))),
                );
        }
        header = header.child(div().flex_1()).child(
            native_button("serial-tools")
                .small()
                .ghost()
                .icon(icons::icon("more-circle"))
                .label("工具")
                .child(icons::icon("down"))
                .dropdown_menu({
                    let owner = cx.entity().downgrade();
                    move |mut m, _, _| {
                        for (key, label) in [
                            ("protocol", "命令与接收规则…"),
                            ("tools", "校验、分帧、Modbus 与采集…"),
                            ("packets", "包库、文件、宏与应答…"),
                        ] {
                            let owner = owner.clone();
                            m = m.item(PopupMenuItem::new(label).on_click(move |_, w, cx| {
                                let _ =
                                    owner.update(cx, |s, cx| s.open_serial_tool(key, label, w, cx));
                            }));
                        }
                        let owner = owner.clone();
                        m.item(
                            PopupMenuItem::new("打开实时曲线")
                                .icon(icons::icon("plot"))
                                .on_click(move |_, _, cx| {
                                    let _ = owner.update(cx, |s, cx| {
                                        if let Some(id) = s.active
                                            && let Err(e) =
                                                s.open_panel(auxiliary::Panel::Plot(id), cx)
                                        {
                                            s.report_error(e.to_string(), cx);
                                        }
                                    });
                                }),
                        )
                    }
                }),
        );
        let mut draft = div().flex_1().min_h_0().px_3().py_2();
        if command {
            draft = draft.child(
                Input::new(&self.fields["command"])
                    .aria_label("协议命令")
                    .disabled(self.installing),
            );
            if let Ok(rules) = serde_json::from_str::<Vec<chipdock_core::serial::program::Rule>>(
                self.editors["protocol"].read(cx).value().as_ref(),
            ) {
                draft = draft.child(
                    div().flex().flex_wrap().gap_1().children(
                        rules
                            .into_iter()
                            .filter(|r| !r.receive)
                            .enumerate()
                            .map(|(i, r)| {
                                native_button(("command-suggestion", i))
                                    .small()
                                    .ghost()
                                    .label(r.name.clone())
                                    .on_click(cx.listener(move |s, _, w, cx| {
                                        s.set("command", format!("{} ", r.name), w, cx)
                                    }))
                            }),
                    ),
                );
            }
        } else {
            draft = draft.child(
                Textarea::new(&self.editors["send"])
                    .aria_label("要发送的数据")
                    .appearance(false)
                    .size_full()
                    .font_family(cx.theme().mono_font_family.clone())
                    .disabled(self.installing),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(draft)
            .child(
                div()
                    .h(px(42.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        native_button("send-history")
                            .small()
                            .ghost()
                            .icon(icons::icon("history"))
                            .tooltip("发送历史")
                            .disabled(if command {
                                self.preferences.command_history.is_empty()
                            } else {
                                self.preferences.history.is_empty()
                            })
                            .dropdown_menu({
                                let owner = cx.entity().downgrade();
                                let entries = if command {
                                    self.preferences.command_history.clone()
                                } else {
                                    self.preferences.history.clone()
                                };
                                move |mut m, _, _| {
                                    for text in &entries {
                                        let value = text.clone();
                                        let owner = owner.clone();
                                        m = m.item(PopupMenuItem::new(text.clone()).on_click(
                                            move |_, w, cx| {
                                                let _ = owner.update(cx, |s, cx| {
                                                    s.set(
                                                        if command { "command" } else { "send" },
                                                        value.clone(),
                                                        w,
                                                        cx,
                                                    )
                                                });
                                            },
                                        ));
                                    }
                                    m
                                }
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child("每"),
                    )
                    .child(
                        div().w(px(64.)).child(
                            Input::new(&self.fields["interval"])
                                .small()
                                .aria_label("自动发送间隔，毫秒"),
                        ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child("ms"),
                    )
                    .child(
                        self.button("repeat", "自动发送", cx, |s, _, cx| {
                            s.toggle_auto_send(cx)
                        })
                        .icon(icons::icon("repeat"))
                        .disabled(command || !self.serial_connected() || self.is_busy()),
                    )
                    .when(
                        self.sessions
                            .iter()
                            .find(|s| Some(s.id) == self.active)
                            .and_then(|s| s.status.as_ref())
                            .is_some_and(|s| s.job_kind.is_some()),
                        |v| {
                            v.child(self.button("cancel-job", "取消任务", cx, |s, _, cx| {
                                s.serial_action("cancelJob", cx)
                            }))
                        },
                    )
                    .child(div().flex_1())
                    .child(
                        self.button("send", "发送", cx, |s, _, cx| {
                            if s.preferences.flag("command_mode") {
                                s.serial_action("command", cx)
                            } else if let Some(id) = s.active {
                                s.submit(Command::Send(id, s.snapshot(cx)), cx)
                            }
                        })
                        .icon(icons::icon("send"))
                        .primary()
                        .disabled(!self.serial_connected() || self.is_busy()),
                    ),
            )
            .into_any_element()
    }
}
