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
        let checksum = self.snapshot(cx).get("checksum").to_owned();
        let mut header = div()
            .h(px(36.))
            .flex_shrink_0()
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(10.))
            .border_b_1()
            .border_color(cx.theme().foreground.opacity(0.1))
            .child(
                div()
                    .flex()
                    .p(px(1.))
                    .rounded(px(6.))
                    .bg(control_bg(cx))
                    .children([(false, "文本"), (true, "命令")].into_iter().map(
                        |(mode, label)| {
                            let selected = command == mode;
                            Button::new(if mode { "command-mode" } else { "normal-mode" })
                                .ghost()
                                .h(px(20.))
                                .px(px(12.))
                                .rounded(px(5.))
                                .toggled(selected)
                                .tooltip("文本：直接发送输入内容；命令：按命令规则生成帧")
                                .when(selected, |b| {
                                    b.bg(cx.theme().primary).text_color(gpui_kit::white())
                                })
                                .child(div().text_size(px(12.)).child(label))
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.preferences.flags.insert("command_mode".into(), mode);
                                    cx.notify();
                                }))
                        },
                    )),
            );
        if !command {
            let label = |v: &str| super::components::option_label(v).to_string();
            header = header
                .child(div().w(px(82.)).child(self.popup_sized(
                    "format",
                    ["utf-8", "hex", "ascii", "gbk", "latin-1"]
                        .into_iter()
                        .map(|v| (v.to_string(), label(v)))
                        .collect(),
                    true,
                    cx,
                )))
                .child(div().w(px(72.)).child(self.popup_sized(
                    "ending",
                    [("none", "无行尾"), ("lf", "LF"), ("cr", "CR"), ("crlf", "CRLF")]
                        .into_iter()
                        .map(|(v, l)| (v.to_string(), l.to_string()))
                        .collect(),
                    true,
                    cx,
                )))
                .child(
                    Button::new("checksum-menu")
                        .secondary()
                        .border_0()
                        .h(px(20.))
                        .px(px(8.))
                        .rounded(px(5.))
                        .tooltip("对编码后的内容计算校验码，在行尾之前追加；奇偶校验追加 0 或 1，CRC16 使用 MODBUS 格式。")
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .text_size(px(12.))
                                .child(icons::icon("shield").size(px(13.)).flex_shrink_0())
                                .child(checksum_title(&checksum))
                                .child(icons::icon("chevron-bold").size(px(8.)).flex_shrink_0().ml(px(4.))),
                        )
                        .dropdown_menu({
                            let owner = cx.entity().downgrade();
                            move |mut m, _, _| {
                                for (value, label) in [
                                    ("none", "无校验（默认）"),
                                    ("sum8", "校验和（SUM8）"),
                                    ("", "奇偶校验"),
                                    ("oddParity", "奇校验"),
                                    ("evenParity", "偶校验"),
                                    ("-", ""),
                                    ("xor8", "异或校验（XOR8）"),
                                    ("crc16Modbus", "CRC16/MODBUS"),
                                    ("crc16Ccitt", "CRC16/CCITT"),
                                    ("crc32", "CRC32"),
                                    ("lrc", "LRC"),
                                ] {
                                    m = match value {
                                        "" => m.separator().label(label),
                                        "-" => m.separator(),
                                        _ => {
                                            let owner = owner.clone();
                                            m.item(
                                                PopupMenuItem::new(label)
                                                    .checked(value == checksum)
                                                    .on_click(move |_, w, cx| {
                                                        let _ = owner.update(cx, |s, cx| {
                                                            s.set("checksum", value, w, cx);
                                                            cx.notify();
                                                        });
                                                    }),
                                            )
                                        }
                                    };
                                }
                                m
                            }
                        }),
                );
        }
        header = header.child(div().flex_1().min_w(px(8.))).child(
            Button::new("serial-tools")
                .ghost()
                .h(px(22.))
                .px(px(4.))
                .tooltip("串口工具")
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .text_size(px(12.))
                        .child(icons::icon("ellipsis-circle").size(px(14.)).flex_shrink_0())
                        .child("工具")
                        .child(icons::icon("chevron-bold").size(px(8.)).flex_shrink_0()),
                )
                .dropdown_menu({
                    let owner = cx.entity().downgrade();
                    move |mut m, _, _| {
                        for (i, (key, label)) in [
                            ("protocol", "命令与接收规则…"),
                            ("tools", "校验、分帧、Modbus 与采集…"),
                            ("packets", "包库、文件、宏与应答…"),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            if i == 1 {
                                m = m.separator();
                            }
                            let owner = owner.clone();
                            m = m.item(PopupMenuItem::new(label).on_click(move |_, w, cx| {
                                let _ =
                                    owner.update(cx, |s, cx| s.open_serial_tool(key, label, w, cx));
                            }));
                        }
                        let owner = owner.clone();
                        m.separator()
                            .item(
                                PopupMenuItem::new("打开实时曲线").on_click(move |_, _, cx| {
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
        let p = self.snapshot(cx);
        let repeating = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref())
            .is_some_and(|s| s.job_kind.as_deref() == Some("repeat"));
        let mut draft = div().relative().flex_1().min_h_0();
        if command {
            draft = draft
                .px(px(14.))
                .py(px(10.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                    let step: isize = match e.keystroke.key.as_str() {
                        "up" => 1,
                        "down" => -1,
                        _ => return,
                    };
                    let history = &s.preferences.command_history;
                    let current = s.snapshot(cx).get("command").to_owned();
                    let index = history.iter().position(|h| *h == current);
                    let next = match index {
                        None if step > 0 => Some(0),
                        None => None,
                        Some(i) => i.checked_add_signed(step),
                    };
                    let value = next
                        .and_then(|i| history.get(i))
                        .cloned()
                        .unwrap_or_default();
                    if !history.is_empty() {
                        s.set("command", value, w, cx);
                        cx.stop_propagation();
                    }
                }))
                .child(
                    Input::new(&self.fields["command"])
                        .small()
                        .font_family(cx.theme().mono_font_family.clone())
                        .aria_label("协议命令")
                        .disabled(self.installing),
                );
            if let Ok(rules) = serde_json::from_str::<Vec<chipdock_core::serial::program::Rule>>(
                self.editors["protocol"].read(cx).value().as_ref(),
            ) {
                let typed = p.get("command").to_owned();
                draft = draft.child(
                    div().flex().flex_wrap().gap(px(4.)).children(
                        rules
                            .into_iter()
                            .filter(|r| {
                                !r.receive && (typed.is_empty() || r.name.starts_with(&typed))
                            })
                            .enumerate()
                            .map(|(i, r)| {
                                Button::new(("command-suggestion", i))
                                    .ghost()
                                    .h(px(20.))
                                    .px(px(8.))
                                    .rounded(px(5.))
                                    .child(div().text_size(px(12.)).child(r.name.clone()))
                                    .on_click(cx.listener(move |s, _, w, cx| {
                                        s.set("command", format!("{} ", r.name), w, cx)
                                    }))
                            }),
                    ),
                );
            }
        } else {
            let empty = self.editors["send"].read(cx).value().is_empty();
            draft = draft
                .px(px(9.))
                .py(px(6.))
                .child(
                    Textarea::new(&self.editors["send"])
                        .aria_label("要发送的数据")
                        .appearance(false)
                        .size_full()
                        .text_size(px(13.))
                        .font_family(cx.theme().mono_font_family.clone())
                        .disabled(self.installing || repeating),
                )
                .when(empty, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(6.))
                            .left(px(14.))
                            .text_size(px(13.))
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().foreground.opacity(0.26))
                            .child(if p.get("format") == "hex" {
                                "输入十六进制字节，例如 AA 01 FF"
                            } else {
                                "输入文本或设备指令…"
                            }),
                    )
                });
        }
        let status = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref());
        let job = status.filter(|s| s.job_kind.as_deref().is_some_and(|k| k != "repeat"));
        let interval_valid = p
            .get("interval")
            .trim()
            .parse::<u64>()
            .is_ok_and(|v| (10..=3_600_000).contains(&v));
        let chars = self.editors["send"].read(cx).value().chars().count();
        let can_send = self.serial_connected() && !self.is_busy();
        let mut footer = div()
            .flex_shrink_0()
            .px(px(12.))
            .py(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .border_t_1()
            .border_color(cx.theme().foreground.opacity(0.1))
            .text_size(px(12.));
        if !command {
            footer = footer
                .child(
                    Button::new("send-history")
                        .ghost()
                        .size(px(20.))
                        .p_0()
                        .text_color(cx.theme().foreground.opacity(0.75))
                        .child(icons::icon("history").size(px(15.)).flex_shrink_0())
                        .tooltip("发送历史")
                        .disabled(self.preferences.history.is_empty() || repeating)
                        .dropdown_menu({
                            let owner = cx.entity().downgrade();
                            let entries = self.preferences.history.clone();
                            move |mut m, _, _| {
                                for text in entries.iter().rev() {
                                    let value = text.clone();
                                    let owner = owner.clone();
                                    m = m.item(
                                        PopupMenuItem::new(
                                            text.chars().take(80).collect::<String>(),
                                        )
                                        .on_click(
                                            move |_, w, cx| {
                                                let _ = owner.update(cx, |s, cx| {
                                                    s.set("send", value.clone(), w, cx)
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
                    div()
                        .w(px(1.))
                        .h(px(14.))
                        .bg(cx.theme().foreground.opacity(0.12)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .text_color(cx.theme().muted_foreground)
                        .child("每")
                        .child(
                            div().w(px(64.)).child(
                                Input::new(&self.fields["interval"])
                                    .small()
                                    .text_right()
                                    .when(!interval_valid, |i| i.text_color(cx.theme().danger))
                                    .disabled(repeating)
                                    .aria_label("自动发送间隔，毫秒"),
                            ),
                        )
                        .child("ms"),
                )
                .child(
                    bordered("repeat", Some("repeat"), "自动发送")
                        .h(px(20.))
                        .px(px(8.))
                        .rounded(px(5.))
                        .toggled(repeating)
                        .when(repeating, |b| {
                            b.bg(cx.theme().primary).text_color(gpui_kit::white())
                        })
                        .tooltip(if repeating {
                            "停止自动发送"
                        } else {
                            "立即发送一次，之后按间隔重复发送；断开连接或发送失败时停止。"
                        })
                        .disabled(!repeating && (!can_send || !interval_valid))
                        .on_click(cx.listener(|s, _, _, cx| s.toggle_auto_send(cx))),
                );
        }
        if let Some(job) = job {
            if job.job_total > 0 {
                footer = footer.child(
                    div().w(px(80.)).child(
                        gpui_kit::component::progress::Progress::new("job-progress")
                            .value(job.job_done as f32 / job.job_total as f32 * 100.),
                    ),
                );
            }
            footer = footer
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{} / {}", job.job_done, job.job_total)),
                )
                .child(
                    bordered("cancel-job", None, "取消任务")
                        .h(px(20.))
                        .px(px(8.))
                        .rounded(px(5.))
                        .on_click(cx.listener(|s, _, _, cx| s.serial_action("cancelJob", cx))),
                );
        }
        footer = footer.child(div().flex_1().min_w(px(4.)));
        if !command && chars > 0 {
            footer = footer.child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{chars} 字符")),
            );
        }
        let sending = self.is_busy() && self.serial_connected();
        footer = footer.child(
            bordered(
                "send",
                Some("paperplane"),
                if sending { "发送中…" } else { "发送" },
            )
            .primary()
            .min_w(px(88.))
            .tooltip(if cfg!(target_os = "macos") {
                "发送数据（⌘Return）"
            } else {
                "发送数据（Ctrl+Enter）"
            })
            .disabled(!can_send || repeating)
            .on_click(cx.listener(|s, _, _, cx| {
                if s.preferences.flag("command_mode") {
                    s.serial_action("command", cx)
                } else if let Some(id) = s.active {
                    s.submit(Command::Send(id, s.snapshot(cx)), cx)
                }
            })),
        );
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(draft)
            .child(footer)
            .into_any_element()
    }
}

pub(super) fn checksum_title(value: &str) -> &'static str {
    match value {
        "sum8" => "校验和",
        "oddParity" => "奇校验",
        "evenParity" => "偶校验",
        "xor8" => "异或校验",
        "crc16Modbus" => "CRC16/MODBUS",
        "crc16Ccitt" => "CRC16/CCITT",
        "crc32" => "CRC32",
        "lrc" => "LRC",
        _ => "无校验",
    }
}
