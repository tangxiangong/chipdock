use super::*;
use gpui_kit::base::input::{FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter};
use std::ops::Range;

pub(super) struct TerminalHighlights(pub Vec<(Range<usize>, u32)>);
impl InputHighlighter for TerminalHighlights {
    fn language(&self) -> SharedString {
        "chipdock-terminal".into()
    }
    fn update(
        &mut self,
        _: Option<InputEdit>,
        _: &ropey::Rope,
        _: bool,
        _: &mut Window,
        _: &mut Context<EditorState>,
    ) {
    }
    fn styles(
        &self,
        range: &Range<usize>,
        _: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let mut result = Vec::new();
        let mut offset = range.start;
        for (span, color) in &self.0 {
            let start = span.start.max(range.start);
            let end = span.end.min(range.end);
            if start >= end {
                continue;
            }
            if offset < start {
                result.push((offset..start, HighlightStyle::default()));
            }
            result.push((
                start..end,
                HighlightStyle {
                    color: Some(rgb(*color).into()),
                    ..Default::default()
                },
            ));
            offset = end;
        }
        if offset < range.end {
            result.push((offset..range.end, HighlightStyle::default()));
        }
        result
    }
    fn fold_ranges(&self, _: &ropey::Rope) -> Vec<FoldRange> {
        vec![]
    }
}
