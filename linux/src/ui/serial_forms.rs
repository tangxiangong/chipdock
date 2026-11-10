use super::*;
impl Workbench {
    pub(super) fn form_group(&self, cx: &App) -> Div {
        div()
            .px_3()
            .py_1()
            .rounded_lg()
            .bg(rgb(if cx.theme().is_dark() {
                0x252525
            } else {
                0xf7f7f7
            }))
            .flex()
            .flex_col()
    }
    pub(super) fn form_input(&self, key: &'static str, label: &'static str, cx: &App) -> Div {
        div()
            .min_h(px(38.))
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().flex_1().child(label))
            .child(
                div().w(px(160.)).child(
                    Input::new(&self.fields[key])
                        .small()
                        .appearance(false)
                        .text_right()
                        .aria_label(label)
                        .disabled(self.installing || (key == "baud" && self.serial_connected())),
                ),
            )
    }
    pub(super) fn form_choice(
        &self,
        key: &'static str,
        label: &'static str,
        values: &'static [&'static str],
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .min_h(px(38.))
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().flex_1().child(label))
            .child(
                div()
                    .w(px(160.))
                    .child(self.compact_choice(key, values, cx)),
            )
    }
    pub(super) fn form_switch(
        &self,
        key: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .min_h(px(38.))
            .flex()
            .items_center()
            .justify_between()
            .child(label)
            .child(
                gpui_kit::component::switch::Switch::new(key)
                    .small()
                    .checked(self.preferences.flag(key))
                    .on_click(cx.listener(move |s, v, _, cx| {
                        s.preferences.flags.insert(key.into(), *v);
                        cx.notify();
                    })),
            )
    }
    pub(super) fn tools_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let tabs = [
            ("checksum", "发送校验"),
            ("framing", "接收分帧"),
            ("modbus", "Modbus RTU"),
            ("capture", "持续采集"),
        ];
        let raw = self.preferences.flag("raw_capture");
        let capturing = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref())
            .is_some_and(|s| if raw { s.capturing } else { s.logging });
        let mut group = self.form_group(cx);
        group = match self.serial_page {
            "framing" => group
                .child(self.form_choice(
                    "frame_mode",
                    "分帧方式",
                    &["read", "idle", "header", "tail", "fixed"],
                    cx,
                ))
                .child(self.form_input("frame_value", "参数", cx))
                .child(self.button("framing", "应用分帧", cx, |s, _, cx| {
                    s.serial_action("framing", cx)
                })),
            "modbus" => group
                .child(self.form_input("station", "站号", cx))
                .child(self.form_choice(
                    "function",
                    "功能码",
                    &["1", "2", "3", "4", "5", "6", "15", "16"],
                    cx,
                ))
                .child(self.form_input("register", "起始地址", cx))
                .child(self.form_input("quantity", "读取数量", cx))
                .when(
                    ["5", "6", "15", "16"]
                        .contains(&self.fields["function"].read(cx).value().as_ref()),
                    |v| v.child(self.form_input("values", "写入值（逗号分隔）", cx)),
                )
                .child(self.form_input("timeout", "超时 (ms)", cx))
                .child(
                    div()
                        .py_3()
                        .flex()
                        .gap_2()
                        .child(self.button("modbus", "生成请求", cx, |s, _, cx| {
                            s.serial_action("modbus", cx)
                        }))
                        .child(
                            self.button("modbus-send", "发送并等待响应", cx, |s, _, cx| {
                                s.serial_action("modbusSend", cx)
                            })
                            .disabled(!self.serial_connected() || self.is_busy()),
                        ),
                ),
            "capture" => group
                .child(self.form_switch("raw_capture", "仅捕获原始 RX", cx))
                .when(!raw, |v| {
                    v.child(self.form_choice(
                        "capture_format",
                        "日志格式",
                        &["csv", "txt", "bin"],
                        cx,
                    ))
                })
                .child(
                    div()
                        .min_h(px(44.))
                        .flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div().flex_1().child(
                                Input::new(&self.fields["capture_path"])
                                    .small()
                                    .appearance(false)
                                    .aria_label("新文件路径"),
                            ),
                        )
                        .child(self.button(
                            "capture-directory",
                            "选择文件夹",
                            cx,
                            |s, _, cx| s.pick_capture_directory(cx),
                        )),
                )
                .child(self.form_input("roll_mib", "按大小滚动 (MiB)", cx))
                .child(
                    div().py_3().flex().gap_2().child(
                        self.button(
                            "capture",
                            if capturing {
                                "停止采集"
                            } else {
                                "开始采集"
                            },
                            cx,
                            move |s, _, cx| {
                                s.serial_action(
                                    match (raw, capturing) {
                                        (true, true) => "stopRaw",
                                        (true, false) => "raw",
                                        (false, true) => "stopCapture",
                                        (false, false) => "capture",
                                    },
                                    cx,
                                );
                            },
                        )
                        .disabled(
                            self.is_busy()
                                || (!capturing && self.snapshot(cx).get("capture_path").is_empty()),
                        ),
                    ),
                ),
            _ => group
                .child(
                    div()
                        .min_h(px(50.))
                        .flex()
                        .items_start()
                        .gap_3()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(div().pt_2().child("试算内容"))
                        .child(
                            Textarea::new(&self.editors["checksum_input"])
                                .flex_1()
                                .appearance(false)
                                .aria_label("试算内容")
                                .h(px(48.)),
                        ),
                )
                .child(self.form_choice(
                    "format",
                    "编码",
                    &["hex", "utf-8", "ascii", "gbk", "latin-1"],
                    cx,
                ))
                .child(self.form_choice(
                    "checksum",
                    "算法",
                    &[
                        "none",
                        "sum8",
                        "xor8",
                        "oddParity",
                        "evenParity",
                        "crc16Modbus",
                        "crc16Ccitt",
                        "crc32",
                        "lrc",
                    ],
                    cx,
                ))
                .child(self.form_input("prefix", "跳过开头字节", cx))
                .child(self.form_input("suffix", "跳过结尾字节", cx))
                .child(self.form_switch("hex_ending", "HEX 发送时允许附加行尾", cx))
                .child(
                    div().py_3().flex().items_start().child(
                        self.button("calculate", "计算", cx, |s, _, cx| {
                            s.serial_action("checksum", cx)
                        })
                        .disabled(self.snapshot(cx).get("checksum_input").is_empty()),
                    ),
                ),
        };
        self.tool_panel(&tabs, group.into_any_element(), cx)
    }
    pub(super) fn tool_panel(
        &self,
        tabs: &[(&'static str, &'static str)],
        body: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .child(
                div()
                    .flex()
                    .pb_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .children(tabs.iter().map(|(key, title)| {
                        let key = *key;
                        native_button(key)
                            .small()
                            .w(px(103.))
                            .label(*title)
                            .when(self.serial_page == key, |b| b.primary())
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.serial_page = key;
                                s.show_text(String::new(), w, cx);
                                cx.notify();
                            }))
                    })),
            )
            .child(
                div()
                    .id("tools-form")
                    .min_h(px(280.))
                    .max_h(px(400.))
                    .overflow_y_scroll()
                    .px_5()
                    .py_4()
                    .child(body),
            );
        if !self.output.read(cx).value().is_empty() {
            panel = panel.child(Textarea::new(&self.output).readonly(true).h(px(100.)));
        }
        panel
            .child(
                div().flex().justify_end().child(
                    native_button("close-tools")
                        .small()
                        .label("完成")
                        .on_click(|_, w, cx| w.close_dialog(cx)),
                ),
            )
            .into_any_element()
    }
}
