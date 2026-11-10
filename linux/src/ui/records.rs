use super::*;
impl Workbench {
    pub(super) fn records_page(&self, cx: &mut Context<Self>) -> Div {
        let p = self.snapshot(cx);
        let frames = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .map(|s| chipdock_gpui::presentation::filtered(s.frames.iter(), &p))
            .transpose();
        let mut rows = match frames {
            Ok(v) => v
                .unwrap_or_default()
                .into_iter()
                .map(|f| {
                    let checksum = f.parsed.iter().filter_map(|p| p.checksum_valid).all(|v| v);
                    (
                        f.sequence,
                        [
                            f.sequence.to_string(),
                            f.timestamp.clone(),
                            match f.direction {
                                Direction::Receive => "RX".into(),
                                Direction::Send => "TX".into(),
                            },
                            f.source.clone(),
                            f.bytes.len().to_string(),
                            f.hex.clone(),
                            crate::serial::frame_text(f, p.get("display")).to_owned(),
                            if f.parsed.iter().any(|p| p.checksum_valid.is_some()) {
                                if checksum {
                                    "通过".into()
                                } else {
                                    "失败".into()
                                }
                            } else {
                                "—".into()
                            },
                        ],
                        serde_json::to_string_pretty(f).unwrap_or_default(),
                    )
                })
                .collect::<Vec<_>>(),
            Err(e) => return div().child(e.to_string()),
        };
        let column = self.record_sort;
        rows.sort_by(|a, b| {
            if column == 0 {
                a.0.cmp(&b.0)
            } else if column == 4 {
                a.1[4]
                    .parse::<usize>()
                    .unwrap_or(0)
                    .cmp(&b.1[4].parse::<usize>().unwrap_or(0))
            } else {
                a.1[column].cmp(&b.1[column])
            }
        });
        if self.record_desc {
            rows.reverse();
        }
        let count = rows.len();
        let selected = self.selected_record;
        let copy = rows
            .iter()
            .find(|r| Some(r.0) == selected)
            .map(|r| r.2.clone());
        let owner = cx.entity().downgrade();
        let widths = [110.; 8];
        let rule = cx.theme().foreground.opacity(0.1);
        let (sort, desc) = (self.record_sort, self.record_desc);
        div()
            .h(px(480.))
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div().flex_1().child(
                            Input::new(&self.fields["filter"])
                                .small()
                                .rounded_full()
                                .aria_label("筛选文本、HEX 或来源")
                                .prefix(icons::icon("search")),
                        ),
                    )
                    .child(self.segmented(
                        &[("all", "全部"), ("receive", "接收"), ("send", "发送")],
                        p.get("direction"),
                        None,
                        cx,
                        |s, key, w, cx| {
                            s.set("direction", key, w, cx);
                            s.render_terminal(w, cx);
                            cx.notify();
                        },
                    ))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{count} 条")),
                    ),
            )
            .child(
                div()
                    .id("record-table-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_x_scroll()
                    .child(
                        div()
                            .min_w(px(880.))
                            .w_full()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .h(px(28.))
                                    .flex_shrink_0()
                                    .flex()
                                    .border_b_1()
                                    .border_color(rule)
                                    .children(
                                        [
                                            "序号", "时间", "方向", "来源", "长度", "HEX", "文本",
                                            "校验",
                                        ]
                                        .into_iter()
                                        .enumerate()
                                        .map(
                                            |(i, name)| {
                                                div()
                                                    .id(("record-column", i))
                                                    .flex_1()
                                                    .min_w(px(widths[i]))
                                                    .h_full()
                                                    .px(px(8.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .border_r_1()
                                                    .border_color(rule)
                                                    .text_size(px(12.))
                                                    .child(name)
                                                    .when(sort == i, |d| {
                                                        d.child(
                                                            icons::icon(if desc { "down" } else { "up" })
                                                                .size(px(10.))
                                                                .text_color(cx.theme().muted_foreground),
                                                        )
                                                    })
                                                    .on_click(cx.listener(move |s, _, _, cx| {
                                                        s.record_desc =
                                                            s.record_sort == i && !s.record_desc;
                                                        s.record_sort = i;
                                                        cx.notify();
                                                    }))
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                uniform_list("records", count.max(18), move |range, _, cx| {
                                    range
                                        .map(|i| {
                                            let mut line =
                                                div().id(("record-row", i)).h(px(24.)).flex().items_center().bg(
                                                    if i % 2 == 0 {
                                                        cx.theme().foreground.opacity(0.04)
                                                    } else {
                                                        cx.theme().background
                                                    },
                                                );
                                            if let Some((sequence, columns, detail)) = rows.get(i) {
                                                let sequence = *sequence;
                                                let detail = detail.clone();
                                                let owner = owner.clone();
                                                line = line
                                                    .bg(if selected == Some(sequence) {
                                                        cx.theme().foreground.opacity(0.14)
                                                    } else if i % 2 == 0 {
                                                        cx.theme().foreground.opacity(0.04)
                                                    } else {
                                                        cx.theme().background
                                                    })
                                                    .children(columns.iter().enumerate().map(
                                                        |(c, value)| {
                                                            div()
                                                                .flex_1()
                                                                .min_w(px(widths[c]))
                                                                .px(px(8.))
                                                                .truncate()
                                                                .overflow_hidden()
                                                                .text_size(px(12.))
                                                                .child(value.clone())
                                                        },
                                                    ))
                                                    .on_click(move |_, w, cx| {
                                                        let _ = owner.update(cx, |s, cx| {
                                                            s.selected_record = Some(sequence);
                                                            s.show_text(detail.clone(), w, cx);
                                                            cx.notify();
                                                        });
                                                    });
                                            }
                                            line.into_any_element()
                                        })
                                        .collect()
                                })
                                .flex_1()
                                .min_h_0(),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        bordered("record-copy", None, "复制")
                            .disabled(copy.is_none())
                            .on_click(move |_, _, cx| {
                                if let Some(text) = &copy {
                                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                                }
                            }),
                    )
                    .child(
                        self.button("locate-record", "在终端定位", cx, |s, w, cx| {
                            if let Some(sequence) = s.selected_record {
                                s.set("filter", "", w, cx);
                                s.set("direction", "all", w, cx);
                                s.preferences.flags.insert("auto_scroll".into(), false);
                                s.render_terminal(w, cx);
                                if let Some(index) =
                                    s.terminal_rows.iter().position(|r| r.sequence == sequence)
                                {
                                    s.terminal_scroll
                                        .scroll_to_item(index, ScrollStrategy::Center);
                                }
                                s.paused = true;
                                w.close_dialog(cx);
                            }
                        })
                        .disabled(selected.is_none()),
                    )
                    .child(div().flex_1())
                    .child(
                        bordered("records-done", None, "完成")
                            .on_click(|_, w, cx| w.close_dialog(cx)),
                    ),
            )
    }
}
