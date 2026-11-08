use super::*;

impl Workbench {
    pub(super) fn terminal_page(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.row(&["filter"]))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(self.flag("search_regex", "正则表达式", cx))
                    .child(self.flag("search_case", "区分大小写", cx)),
            )
            .child(self.choice("direction", &["all", "receive", "send"], window, cx))
            .child(self.choice(
                "display",
                &["utf-8", "hex", "ascii", "gbk", "latin-1"],
                window,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(self.flag("auto_scroll", "自动滚动", cx))
                    .child(self.flag("timestamps", "时间戳", cx))
                    .child(self.flag("mixed", "文本与 HEX", cx))
                    .child(self.flag("reset_on_clear", "清空时重置计数", cx)),
            )
            .child(
                Editor::new(&self.terminal)
                    .font_family(cx.theme().mono_font_family.clone())
                    .h(px(240.))
                    .readonly(true),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.button("pause", "暂停／继续显示", cx, |s, _, cx| {
                        s.paused = !s.paused;
                        cx.notify();
                    }))
                    .child(self.button("clear", "清空显示", cx, |s, w, cx| {
                        s.clear_terminal(w, cx)
                    }))
                    .child(self.button("copy-terminal", "复制终端", cx, |s, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            s.terminal.read(cx).value().to_string(),
                        ))
                    }))
                    .child(self.button("reset-count", "重置计数", cx, |s, _, cx| {
                        s.serial_action("resetCounters", cx)
                    })),
            )
            .child(self.field("send"))
            .child(
                div().flex().flex_wrap().gap_1().children(
                    self.preferences
                        .history
                        .iter()
                        .take(12)
                        .enumerate()
                        .map(|(index, text)| {
                            let text = text.clone();
                            Button::new(("history", index))
                                .small()
                                .label(text.clone())
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    s.set("send", text.clone(), w, cx)
                                }))
                        }),
                ),
            )
            .child(self.choice(
                "format",
                &["utf-8", "hex", "ascii", "gbk", "latin-1"],
                window,
                cx,
            ))
            .child(self.choice("ending", &["none", "lf", "cr", "crlf"], window, cx))
            .child(self.choice(
                "checksum",
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
                window,
                cx,
            ))
            .child(self.row(&["prefix", "suffix", "interval"]))
            .child(self.flag("hex_ending", "HEX 发送附加行尾", cx))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        self.button("send", "发送", cx, |s, _, cx| {
                            if let Some(id) = s.active {
                                s.submit(Command::Send(id, s.snapshot(cx)), cx)
                            }
                        })
                        .primary(),
                    )
                    .child(self.button("repeat", "循环发送", cx, |s, _, cx| {
                        s.serial_action("repeat", cx)
                    })),
            )
    }
}

impl Workbench {
    pub(super) fn records_page(&self, cx: &mut Context<Self>) -> Div {
        let p = self.snapshot(cx);
        let filtered = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .map(|s| chipdock_gpui::presentation::filtered(s.frames.iter(), &p))
            .transpose();
        let frames = match filtered {
            Ok(frames) => frames.unwrap_or_default(),
            Err(error) => return div().child(format!("搜索表达式无效：{error}")),
        };
        let rules =
            match chipdock_gpui::presentation::Colors::new(p.get("colors"), p.get("tx_color")) {
                Ok(rules) => rules,
                Err(error) => {
                    return div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(format!("着色规则无效：{error}"))
                        .child(self.field("tx_color"))
                        .child(self.editor("colors", cx));
                }
            };
        let export = frames.iter().map(|f| (*f).clone()).collect::<Vec<_>>();
        let rows = frames
            .into_iter()
            .map(|frame| {
                let text = crate::serial::frame_text(frame, p.get("display"));
                (
                    format!(
                        "#{}  {}  {:?}  {}",
                        frame.sequence,
                        frame.timestamp,
                        frame.direction,
                        text.replace('\n', " ↵ ")
                    ),
                    rules.for_frame(frame, text),
                    serde_json::to_string_pretty(frame).unwrap_or_default(),
                    frame.sequence,
                )
            })
            .collect::<Vec<_>>();
        let view = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.field("filter"))
            .child(div().h(px(280.)).child(uniform_list(
                "serial-records",
                rows.len(),
                move |range, _, cx| {
                    range
                        .map(|index| {
                            let (label, color, detail, sequence) = &rows[index];
                            let sequence = *sequence;
                            let detail = detail.clone();
                            let view = view.clone();
                            div()
                                .id(("record", index))
                                .h(px(28.))
                                .px_2()
                                .text_sm()
                                .overflow_hidden()
                                .text_color(
                                    color
                                        .map(rgb)
                                        .map(Into::into)
                                        .unwrap_or(cx.theme().foreground),
                                )
                                .child(label.clone())
                                .on_click(move |_, w, cx| {
                                    let _ = view.update(cx, |s, cx| {
                                        s.selected_record = Some(sequence);
                                        s.show_text(detail.clone(), w, cx)
                                    });
                                })
                        })
                        .collect()
                },
            )))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .children(["json", "txt", "csv", "bin"].into_iter().map(|format| {
                        let frames = export.clone();
                        let encoding = p.get("display").to_owned();
                        Button::new(SharedString::from(format!("records-export-{format}")))
                            .label(format!("导出 {format}…"))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                match chipdock_gpui::presentation::records(
                                    &frames.iter().collect::<Vec<_>>(),
                                    format,
                                    &encoding,
                                ) {
                                    Ok(bytes) => s.export_bytes(
                                        &format!("ChipDock-records.{format}"),
                                        bytes,
                                        cx,
                                    ),
                                    Err(e) => s.report_error(e.to_string(), cx),
                                }
                            }))
                    })),
            )
            .child(self.button(
                "locate-record",
                "在终端定位选中记录",
                cx,
                |s, w, cx| {
                    if let Some(id) = s.selected_record {
                        s.set("filter", "", w, cx);
                        s.set("direction", "all", w, cx);
                        s.paused = true;
                        s.serial_page = "terminal";
                        s.preferences.flags.insert("auto_scroll".into(), false);
                        s.render_terminal(w, cx);
                        if let Some(range) = s.terminal_ranges.get(&id).cloned() {
                            s.terminal
                                .update(cx, |t, cx| t.set_selected_range(range, cx));
                        } else {
                            s.report("该记录已超出显示保留范围", cx);
                        }
                    }
                },
            ))
            .child(self.field("tx_color"))
            .child(self.editor("colors", cx))
    }
}
