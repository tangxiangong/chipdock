use super::*;
impl Workbench {
    pub(super) fn logs_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.fields["log_search"].read(cx).value().to_lowercase();
        let entries = self
            .logs
            .iter()
            .filter(|s| s.to_lowercase().contains(&query))
            .filter(|s| !self.preferences.flag("errors_only") || s.contains("[错误]"))
            .cloned()
            .collect::<Vec<_>>();
        let copy = entries.join("\n");
        let export = copy.clone().into_bytes();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("操作日志"))
            .child(
                self.button("detach-logs", "在独立窗口打开", cx, |s, _, cx| {
                    if let Err(e) = s.open_panel(super::auxiliary::Panel::Logs, cx) {
                        s.report_error(e.to_string(), cx);
                    }
                }),
            )
            .child(self.field("log_search"))
            .child(self.flag("follow_logs", "跟随最新日志", cx))
            .child(self.flag("errors_only", "仅显示错误", cx))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("copy-logs")
                            .label("复制日志")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                            }),
                    )
                    .child(
                        self.button("export-logs", "导出日志…", cx, move |s, _, cx| {
                            s.export_bytes("ChipDock.log", export.clone(), cx)
                        }),
                    )
                    .child(self.button("clear-logs", "清空日志", cx, |s, _, cx| {
                        s.logs.clear();
                        s.unseen_errors = 0;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("logs-list")
                    .h(px(450.))
                    .overflow_y_scroll()
                    .track_scroll(&self.logs_scroll)
                    .children(entries.into_iter().map(|s| div().text_sm().child(s))),
            )
            .into_any_element()
    }
}
