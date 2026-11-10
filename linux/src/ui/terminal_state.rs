use super::*;

impl Workbench {
    pub(super) fn render_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let p = self.snapshot(cx);
        let filter = match crate::serial::search(&p) {
            Ok(filter) => filter,
            Err(error) => {
                self.message = format!("搜索表达式无效：{error}");
                cx.notify();
                return;
            }
        };
        let colors =
            match chipdock_gpui::presentation::Colors::new(p.get("colors"), p.get("tx_color")) {
                Ok(colors) => colors,
                Err(error) => {
                    self.message = format!("着色规则无效：{error}");
                    cx.notify();
                    return;
                }
            };
        let mut rows = Vec::new();
        let mut text = String::new();
        let mut highlights = Vec::new();
        self.terminal_ranges.clear();
        if let Some(session) = self.sessions.iter().find(|s| Some(s.id) == self.active) {
            for frame in &session.frames {
                if !chipdock_gpui::presentation::matches(frame, &p, &filter) {
                    continue;
                }
                let value = chipdock_gpui::presentation::display_text(frame, &p);
                rows.push(super::terminal_table::Row {
                    sequence: frame.sequence,
                    timestamp: frame.timestamp.clone(),
                    direction: match frame.direction {
                        Direction::Receive => "RX",
                        Direction::Send => "TX",
                    },
                    bytes: frame.bytes.len(),
                    text: value.replace('\n', " ↵ "),
                    color: colors.for_frame(frame, &value),
                });
                let start = text.len();
                if p.flag("timestamps") {
                    text.push_str(&frame.timestamp);
                    text.push_str("  ");
                }
                text.push_str(match frame.direction {
                    Direction::Receive => "RX  ",
                    Direction::Send => "TX  ",
                });
                text.push_str(&value);
                if !frame.parsed.is_empty() {
                    text.push_str(&format!(
                        "\n  {}",
                        serde_json::to_string(&frame.parsed).unwrap_or_default()
                    ));
                }
                self.terminal_ranges
                    .insert(frame.sequence, start..text.len());
                if let Some(color) = colors.for_frame(frame, &value) {
                    highlights.push((start..text.len(), color));
                }
                text.push('\n');
            }
        }
        if p.flag("auto_scroll") && !rows.is_empty() {
            self.terminal_scroll
                .scroll_to_item(rows.len() - 1, ScrollStrategy::Bottom);
        }
        self.terminal_rows = std::rc::Rc::new(rows);
        let style = format!("{}:{}", p.get("colors"), p.get("tx_color"));
        let style_changed = style != self.terminal_style;
        self.terminal_style = style;
        if self.terminal.read(cx).value().as_ref() != text || style_changed {
            self.terminal.update(cx, |s, cx| {
                let previous = s.value();
                let selection = s.selected_range();
                let offset = s.scroll_offset();
                let append = text.starts_with(previous.as_ref());
                s.set_highlighter_factory(
                    std::rc::Rc::new(move |_| {
                        Some(Box::new(super::highlights::TerminalHighlights(
                            highlights.clone(),
                        )))
                    }),
                    cx,
                );
                s.set_value(text, window, cx);
                if append {
                    s.set_selected_range(selection, cx);
                }
                s.set_scroll_offset(
                    if p.flag("auto_scroll") {
                        point(px(0.), px(-1_000_000_000.))
                    } else {
                        offset
                    },
                    cx,
                );
            });
        }
    }
}
