use super::*;

impl Workbench {
    pub(super) fn connection_panel(&self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let _ = w;
        let connected = self.serial_connected();
        let status = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref());
        let reconnecting = status.is_some_and(|s| s.reconnecting);
        let busy = self.is_busy();
        let locked = connected || reconnecting;
        let p = self.snapshot(cx);
        let mut options = self
            .ports
            .iter()
            .map(|p| (p.path.clone(), p.display_name.clone()))
            .collect::<Vec<_>>();
        let selected = p.get("port").to_owned();
        if !selected.is_empty() && !options.iter().any(|(path, _)| *path == selected) {
            options.push((selected.clone(), format!("{selected}（已移除）")));
        }
        if options.is_empty() {
            options.push((String::new(), "未发现串口".into()));
        }
        let separator = cx.theme().foreground.opacity(0.08);
        let section = |title: &'static str| {
            div().flex().flex_col().gap(px(10.)).child(
                div()
                    .px(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
        };
        let row = |label: &'static str, last: bool| {
            div()
                .min_h(px(40.))
                .flex()
                .items_center()
                .gap(px(20.))
                .when(!last, |d| d.border_b_1().border_color(separator))
                .child(div().flex_shrink_0().child(label))
        };
        let baud_valid = p.get("baud").trim().parse::<u32>().is_ok_and(|v| v > 0);
        let mut parameters = section("参数").child(
            self.form_group(cx)
                .child(
                    row("波特率", false).child(div().flex_1()).child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap(px(2.))
                            .child(
                                div().w(px(90.)).child(
                                    Input::new(&self.fields["baud"])
                                        .small()
                                        .appearance(false)
                                        .text_right()
                                        .aria_label("波特率")
                                        .disabled(self.installing || locked),
                                ),
                            )
                            .child(self.baud_menu(locked, cx)),
                    ),
                )
                .child(row("数据位", false).child(self.serial_popup(
                    "bits",
                    &["5", "6", "7", "8"],
                    cx,
                )))
                .child(row("停止位", false).child(self.serial_popup("stop", &["1", "2"], cx)))
                .child(row("校验位", false).child(self.serial_popup(
                    "parity",
                    &["none", "even", "odd"],
                    cx,
                )))
                .child(row("流控制", true).child(self.serial_popup(
                    "flow",
                    &["none", "software", "hardware"],
                    cx,
                ))),
        );
        if !baud_valid {
            parameters = parameters.child(
                div()
                    .px(px(10.))
                    .text_size(px(10.))
                    .text_color(cx.theme().danger)
                    .child("波特率须为正整数"),
            );
        }
        let mut controls = div()
            .id("connection-settings")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_5()
            .flex()
            .flex_col()
            .gap(px(30.))
            .child(
                section("设备").child(
                    self.form_group(cx).child(
                        div()
                            .min_h(px(39.))
                            .flex()
                            .gap(px(4.))
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(self.serial_popup_with("port", options, cx)),
                            )
                            .child(
                                Button::new("ports")
                                    .ghost()
                                    .size(px(20.))
                                    .p_0()
                                    .text_color(cx.theme().foreground.opacity(0.7))
                                    .child(icons::icon("refresh").size(px(14.)).flex_shrink_0())
                                    .tooltip(if cfg!(target_os = "macos") {
                                        "刷新串口（⌘R）"
                                    } else {
                                        "刷新串口（Ctrl+R）"
                                    })
                                    .disabled(self.busy_lanes.contains(&Lane::Serial))
                                    .on_click(
                                        cx.listener(|s, _, _, cx| s.submit(Command::Ports, cx)),
                                    ),
                            ),
                    ),
                ),
            )
            .child(parameters)
            .child(
                section("选项").child(
                    self.form_group(cx)
                        .child(self.form_switch("auto_connect", "启动时连接", cx))
                        .child(self.form_switch("reconnect", "掉线后重连", cx)),
                ),
            );
        if let Some(status) = status.filter(|s| {
            s.connected && !s.path.as_deref().is_some_and(|p| p.starts_with("VIRTUAL-"))
        }) {
            let hardware_flow = status
                .config
                .as_ref()
                .is_some_and(|c| matches!(c.flow_control, FlowControl::Hardware));
            let mut lines = self.form_group(cx);
            for (i, (line, label, value)) in [
                (ControlLine::Rts, "RTS", status.rts),
                (ControlLine::Dtr, "DTR", status.dtr),
            ]
            .into_iter()
            .enumerate()
            {
                let disabled = busy || (i == 0 && hardware_flow);
                lines = lines.child(
                    row(label, i == 1).child(div().flex_1()).child(
                        div()
                            .flex()
                            .p(px(2.))
                            .rounded(px(7.))
                            .bg(control_bg(cx))
                            .children([(false, "清除"), (true, "置位")].into_iter().map(
                                |(level, text)| {
                                    let on = value.unwrap_or(false) == level;
                                    Button::new((label, level as usize))
                                        .ghost()
                                        .h(px(20.))
                                        .px(px(10.))
                                        .rounded(px(5.))
                                        .disabled(disabled)
                                        .when(on, |b| {
                                            b.bg(rgb(if cx.theme().is_dark() {
                                                0x5a5a5a
                                            } else {
                                                0xd6d6d6
                                            }))
                                        })
                                        .child(div().text_size(px(12.)).child(text))
                                        .on_click(cx.listener(move |s, _, _, cx| {
                                            if let Some(id) = s.active {
                                                s.submit(Command::Line(id, line, level), cx)
                                            }
                                        }))
                                },
                            )),
                    ),
                );
            }
            controls = controls.child(section("控制线").child(lines));
        }
        let can_connect = !self.fields["port"].read(cx).value().is_empty() && baud_valid;
        let title = if busy {
            "正在处理…"
        } else if reconnecting {
            "取消重连"
        } else if connected {
            "断开连接"
        } else {
            "连接串口"
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(controls)
            .child(
                div()
                    .p(px(12.))
                    .border_t_1()
                    .border_color(cx.theme().foreground.opacity(0.1))
                    .child(
                        prominent(
                            "serial-connect",
                            (!busy).then_some(if connected { "stop" } else { "link" }),
                            title,
                        )
                        .w_full()
                        .tooltip(if cfg!(target_os = "macos") {
                            "连接或断开串口（⌘K）"
                        } else {
                            "连接或断开串口（Ctrl+K）"
                        })
                        .when(busy, |b| {
                            b.icon(gpui_kit::component::spinner::Spinner::new().small())
                        })
                        .disabled(
                            busy || self.installing
                                || (!connected && !reconnecting && !can_connect),
                        )
                        .on_click(cx.listener(|s, _, _, cx| {
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
                        })),
                    ),
            )
            .into_any_element()
    }
    /// `NSPopUpButton` at the small control size, filling its row.
    fn serial_popup(
        &self,
        key: &'static str,
        values: &'static [&'static str],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.serial_popup_with(
            key,
            values
                .iter()
                .map(|v| {
                    (
                        v.to_string(),
                        match (key, *v) {
                            ("flow", "software") => "XON/XOFF".into(),
                            ("flow", "hardware") => "RTS/CTS".into(),
                            _ => super::components::option_label(v).to_string(),
                        },
                    )
                })
                .collect(),
            cx,
        )
    }
    fn serial_popup_with(
        &self,
        key: &'static str,
        options: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex_1()
            .w_full()
            .min_w_0()
            .child(self.popup_sized(key, options, true, cx))
            .into_any_element()
    }
    fn baud_menu(&self, locked: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        Button::new("baud-presets")
            .ghost()
            .size(px(18.))
            .p_0()
            .disabled(locked || self.installing)
            .child(icons::icon("select").size(px(13.)).flex_shrink_0())
            .tooltip("选择常用波特率")
            .dropdown_menu(move |mut m, _, _| {
                for rate in [
                    1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
                ] {
                    let owner = owner.clone();
                    m = m.item(
                        PopupMenuItem::new(rate.to_string()).on_click(move |_, w, cx| {
                            let _ = owner.update(cx, |s, cx| {
                                s.set("baud", rate.to_string(), w, cx);
                                cx.notify();
                            });
                        }),
                    );
                }
                m
            })
    }
    pub(super) fn serial_status_bar(&self, cx: &App) -> Div {
        let busy = self.is_busy();
        let mut bar = div()
            .h(px(26.))
            .flex_shrink_0()
            .px(px(14.))
            .flex()
            .items_center()
            .gap(px(10.))
            .border_t_1()
            .border_color(cx.theme().foreground.opacity(0.1))
            .text_size(px(10.))
            .text_color(cx.theme().muted_foreground);
        let separator = || {
            div()
                .w(px(1.))
                .h(px(12.))
                .bg(cx.theme().foreground.opacity(0.12))
        };
        let Some(s) = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref())
        else {
            return bar;
        };
        bar = bar.child(
            div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(6.))
                .child(div().size(px(7.)).rounded_full().bg(if s.connected {
                    rgb(0x34c759).into()
                } else if s.reconnecting {
                    rgb(0xff9500).into()
                } else {
                    cx.theme().muted_foreground.opacity(0.4)
                }))
                .child(div().text_color(cx.theme().foreground).child(if busy {
                    "正在处理…"
                } else if s.reconnecting {
                    "正在重连…"
                } else if s.connected {
                    "已连接"
                } else {
                    "未连接"
                })),
        );
        if s.connected
            && let Some(path) = &s.path
        {
            bar = bar
                .child(separator())
                .child(div().min_w_0().truncate().child(path.clone()));
            if let Some(config) = &s.config {
                bar = bar
                    .child(separator())
                    .child(div().truncate().child(config_summary(config)));
            }
        }
        bar = bar.child(div().flex_1().min_w(px(8.)));
        if s.logging || s.capturing {
            bar = bar
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .text_color(cx.theme().danger)
                        .child(
                            div()
                                .size(px(8.))
                                .rounded_full()
                                .border_1()
                                .border_color(cx.theme().danger),
                        )
                        .child("采集中"),
                )
                .child(separator());
        }
        if s.connected {
            let secs = s.uptime_ms / 1000;
            bar = bar
                .child(format!(
                    "{}:{:02}:{:02}",
                    secs / 3600,
                    secs / 60 % 60,
                    secs % 60
                ))
                .child(separator())
                .child(format!("{} / {} B/s", s.rx_per_second, s.tx_per_second))
                .child(separator());
        }
        bar.child(
            div()
                .flex()
                .items_center()
                .gap(px(3.))
                .child(icons::icon("arrow-down").size(px(11.)))
                .child(format!("{} B", grouped(s.rx_bytes))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(3.))
                .child(icons::icon("arrow-up").size(px(11.)))
                .child(format!("{} B", grouped(s.tx_bytes))),
        )
    }
}

fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn config_summary(config: &SerialConfig) -> String {
    format!(
        "{} · {}{}{}",
        config.baud_rate,
        config.data_bits,
        match config.parity {
            Parity::Even => "E",
            Parity::Odd => "O",
            _ => "N",
        },
        config.stop_bits
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn byte_counts_use_thousands_separators() {
        assert_eq!(super::grouped(0), "0");
        assert_eq!(super::grouped(1234), "1,234");
        assert_eq!(super::grouped(1234567), "1,234,567");
    }
}
