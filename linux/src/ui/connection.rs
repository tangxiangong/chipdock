use super::*;

impl Workbench {
    pub(super) fn connection_panel(&self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let connected = self.serial_connected();
        let mut options = vec![(String::new(), "选择串口设备".into())];
        options.extend(
            self.ports
                .iter()
                .map(|p| (p.path.clone(), p.display_name.clone())),
        );
        let mut controls = div()
            .id("connection-settings")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_5()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().px_2().font_weight(FontWeight::SEMIBOLD).child("设备"))
                    .child(
                        self.form_group(cx).child(
                            div()
                                .h(px(38.))
                                .flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(self.popup("port", options, cx)),
                                )
                                .child(
                                    native_button("ports")
                                        .small()
                                        .ghost()
                                        .icon(icons::icon("refresh"))
                                        .tooltip("刷新串口")
                                        .on_click(
                                            cx.listener(|s, _, _, cx| s.submit(Command::Ports, cx)),
                                        ),
                                ),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().px_2().font_weight(FontWeight::SEMIBOLD).child("参数"))
                    .child(
                        self.form_group(cx)
                            .child(self.form_input("baud", "波特率", cx))
                            .child(self.form_choice("bits", "数据位", &["5", "6", "7", "8"], cx))
                            .child(self.form_choice("stop", "停止位", &["1", "15", "2"], cx))
                            .child(self.form_choice(
                                "parity",
                                "校验位",
                                &["none", "odd", "even", "mark", "space"],
                                cx,
                            ))
                            .child(self.form_choice(
                                "flow",
                                "流控制",
                                &["none", "software", "hardware"],
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().px_2().font_weight(FontWeight::SEMIBOLD).child("选项"))
                    .child(
                        self.form_group(cx)
                            .child(self.form_switch("auto_connect", "启动时连接", cx))
                            .child(self.form_switch("reconnect", "掉线后重连", cx)),
                    ),
            );
        if connected
            && let Some(status) = self
                .sessions
                .iter()
                .find(|s| Some(s.id) == self.active)
                .and_then(|s| s.status.as_ref())
        {
            for (line, label, value) in [
                (ControlLine::Rts, "RTS", status.rts),
                (ControlLine::Dtr, "DTR", status.dtr),
            ] {
                controls = controls.child(
                    native_button(label)
                        .small()
                        .label(format!(
                            "{label}：{}",
                            value.map_or("未知", |v| if v { "高" } else { "低" })
                        ))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            if let Some(id) = s.active {
                                s.submit(Command::Line(id, line, !value.unwrap_or(false)), cx)
                            }
                        })),
                );
            }
        }
        let _ = w;
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(controls)
            .child(
                div()
                    .h(px(52.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        self.button(
                            "serial-connect",
                            if connected {
                                "断开串口"
                            } else {
                                "连接串口"
                            },
                            cx,
                            |s, _, cx| {
                                if let Some(id) = s.active {
                                    s.submit(
                                        if s.serial_connected() {
                                            Command::Disconnect(id)
                                        } else {
                                            Command::ConnectSerial(id, s.snapshot(cx))
                                        },
                                        cx,
                                    )
                                }
                            },
                        )
                        .icon(icons::icon(if connected { "unlink" } else { "link" }))
                        .w_full()
                        .h(px(28.))
                        .rounded_full()
                        .disabled(
                            self.is_busy()
                                || (!connected && self.fields["port"].read(cx).value().is_empty()),
                        ),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn serial_status_bar(&self, cx: &App) -> Div {
        let mut bar = div()
            .h(px(26.))
            .flex_shrink_0()
            .px_4()
            .flex()
            .items_center()
            .gap_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .text_xs()
            .text_color(cx.theme().muted_foreground);
        if let Some(s) = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref())
        {
            bar = bar
                .child(
                    div()
                        .size(px(7.))
                        .rounded_full()
                        .border_1()
                        .border_color(cx.theme().muted_foreground),
                )
                .child(if s.connected {
                    "已连接"
                } else if s.reconnecting {
                    "正在重连…"
                } else {
                    "未连接"
                })
                .child(div().flex_1())
                .child(if s.logging || s.capturing {
                    "采集中"
                } else {
                    ""
                })
                .child(icons::icon("arrow-down").size(px(12.)))
                .child(format!("RX {} B", s.rx_bytes))
                .child(icons::icon("arrow-up").size(px(12.)))
                .child(format!("TX {} B", s.tx_bytes));
        }
        bar
    }
}
