use super::*;

pub(super) struct Row {
    pub sequence: u64,
    pub timestamp: String,
    pub direction: &'static str,
    pub bytes: usize,
    pub text: String,
    pub color: Option<u32>,
}

impl Workbench {
    pub(super) fn terminal_table(&self, cx: &mut Context<Self>) -> AnyElement {
        let timestamps = self.preferences.flag("timestamps");
        let rule = cx.theme().foreground.opacity(0.1);
        let column = |title: &'static str, width: Option<f32>| {
            div()
                .h_full()
                .flex()
                .items_center()
                .px(px(6.))
                .border_r_1()
                .border_color(rule)
                .map(|d| match width {
                    Some(w) => d.w(px(w + 12.)).flex_shrink_0(),
                    None => d.flex_1().border_r_0(),
                })
                .child(div().pl(px(4.)).child(title))
        };
        let mut header = div()
            .flex()
            .h(px(28.))
            .flex_shrink_0()
            .px(px(2.))
            .border_b_1()
            .border_color(rule)
            .text_size(px(12.));
        if timestamps {
            header = header.child(column("时间", Some(104.)));
        }
        header = header
            .child(column("方向", Some(64.)))
            .child(column("字节", Some(52.)))
            .child(column("数据", None));
        let mut table = div().flex_1().min_h_0().flex().flex_col().child(header);
        if self.terminal_rows.is_empty() {
            let total = self
                .sessions
                .iter()
                .find(|s| Some(s.id) == self.active)
                .map_or(0, |s| s.frames.len());
            let filtered = total > 0;
            let connected = self.serial_connected();
            let (icon, title, detail) = if filtered {
                (
                    "search",
                    "没有匹配的数据",
                    "尝试其他搜索内容，或显示全部方向。",
                )
            } else if connected {
                ("antenna", "等待设备数据", "设备数据会实时显示在这里。")
            } else {
                (
                    "cable",
                    "暂无收发记录",
                    "在左侧选择串口并连接，即可开始收发。",
                )
            };
            table = table.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .text_color(cx.theme().muted_foreground)
                    .child(icons::icon(icon).size(px(44.)).mb(px(4.)))
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(FontWeight::BOLD)
                            .child(title),
                    )
                    .child(div().text_size(px(13.)).child(detail))
                    .when(filtered, |d| {
                        d.child(div().mt(px(4.)).child(self.button(
                            "clear-filter",
                            "清除筛选",
                            cx,
                            |s, w, cx| {
                                s.set("filter", "", w, cx);
                                s.set("direction", "all", w, cx);
                                s.render_terminal(w, cx);
                            },
                        )))
                    }),
            );
        } else {
            let rows = self.terminal_rows.clone();
            let owner = cx.entity().downgrade();
            let selected = self.selected_record;
            table = table.child(
                uniform_list("terminal-table", rows.len(), move |range, _, cx| {
                    range
                        .map(|i| {
                            let row = &rows[i];
                            let owner = owner.clone();
                            let sequence = row.sequence;
                            let secondary = cx.theme().muted_foreground;
                            let mono = cx.theme().mono_font_family.clone();
                            let receive = row.direction == "RX";
                            let cell = |width: f32| {
                                div()
                                    .w(px(width + 12.))
                                    .flex_shrink_0()
                                    .px(px(10.))
                                    .pt(px(6.))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                            };
                            let mut line = div()
                                .id(("terminal-row", i))
                                .h(px(32.))
                                .px(px(2.))
                                .flex()
                                .items_start()
                                .font_family(mono)
                                .text_size(px(11.))
                                .overflow_hidden()
                                .when(selected == Some(sequence), |d| {
                                    d.bg(cx.theme().foreground.opacity(0.1))
                                });
                            if timestamps {
                                line = line.child(
                                    cell(104.)
                                        .text_color(secondary)
                                        .child(row.timestamp.clone()),
                                );
                            }
                            line.child(
                                cell(64.)
                                    .pl(px(8.))
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        icons::icon(if receive {
                                            "arrow-down-left"
                                        } else {
                                            "arrow-up-right"
                                        })
                                        .size(px(11.))
                                        .flex_shrink_0()
                                        .text_color(cx.theme().primary),
                                    )
                                    .child(row.direction),
                            )
                            .child(
                                cell(52.)
                                    .text_right()
                                    .text_color(secondary)
                                    .child(row.bytes.to_string()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .px(px(10.))
                                    .pt(px(5.))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_size(px(13.))
                                    .text_color(
                                        row.color
                                            .map(rgb)
                                            .map(Into::into)
                                            .unwrap_or(cx.theme().foreground),
                                    )
                                    .child(row.text.clone()),
                            )
                            .on_click(move |event, w, cx| {
                                let _ = owner.update(cx, |s, cx| {
                                    s.selected_record = Some(sequence);
                                    if event.click_count() == 2
                                        && let Some(frame) = s
                                            .sessions
                                            .iter()
                                            .find(|session| Some(session.id) == s.active)
                                            .and_then(|s| {
                                                s.frames.iter().find(|f| f.sequence == sequence)
                                            })
                                    {
                                        s.show_text(
                                            serde_json::to_string_pretty(frame).unwrap_or_default(),
                                            w,
                                            cx,
                                        );
                                        let output = s.output.clone();
                                        s.focus_handle.focus(w, cx);
                                        w.open_dialog(cx, move |d, _, _| {
                                            d.title("收发记录").w(px(640.)).centered().child(
                                                Textarea::new(&output).h(px(300.)).readonly(true),
                                            )
                                        });
                                    }
                                    cx.notify();
                                });
                            })
                            .into_any_element()
                        })
                        .collect()
                })
                .track_scroll(&self.terminal_scroll)
                .flex_1()
                .min_h_0(),
            );
        }
        table.into_any_element()
    }
}
