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
        let mut header = div()
            .flex()
            .h(px(28.))
            .items_center()
            .px_3()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .text_size(px(12.))
            .text_color(cx.theme().muted_foreground);
        if timestamps {
            header = header.child(div().w(px(104.)).child("时间"));
        }
        header = header
            .child(div().w(px(64.)).child("方向"))
            .child(div().w(px(56.)).child("字节"))
            .child(div().flex_1().child("数据"));
        let mut table = div().flex_1().min_h_0().flex().flex_col().child(header);
        if self.terminal_rows.is_empty() {
            table = table.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        icons::icon("usb")
                            .size(px(32.))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("暂无收发记录"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child("在左侧选择串口并连接，即可开始收发。"),
                    ),
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
                            let mut line = div()
                                .id(("terminal-row", i))
                                .h(px(26.))
                                .px_3()
                                .flex()
                                .items_center()
                                .gap_3()
                                .text_size(px(12.))
                                .font_family(cx.theme().mono_font_family.clone())
                                .overflow_hidden()
                                .bg(if selected == Some(sequence) {
                                    cx.theme().accent
                                } else {
                                    cx.theme().background
                                })
                                .text_color(
                                    row.color
                                        .map(rgb)
                                        .map(Into::into)
                                        .unwrap_or(cx.theme().foreground),
                                );
                            if timestamps {
                                line = line.child(
                                    div()
                                        .w(px(104.))
                                        .flex_shrink_0()
                                        .overflow_hidden()
                                        .child(row.timestamp.clone()),
                                );
                            }
                            line.child(div().w(px(64.)).flex_shrink_0().child(row.direction))
                                .child(
                                    div()
                                        .w(px(56.))
                                        .flex_shrink_0()
                                        .child(row.bytes.to_string()),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
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
                                                serde_json::to_string_pretty(frame)
                                                    .unwrap_or_default(),
                                                w,
                                                cx,
                                            );
                                            let output = s.output.clone();
                                            s.focus_handle.focus(w, cx);
                                            w.open_dialog(cx, move |d, _, _| {
                                                d.title("收发记录").w(px(640.)).child(
                                                    Textarea::new(&output)
                                                        .h(px(300.))
                                                        .readonly(true),
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
