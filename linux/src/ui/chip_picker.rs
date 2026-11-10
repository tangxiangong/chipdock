use super::*;
use std::sync::Arc;

type SelectionHandler = Box<dyn Fn(Option<String>, &mut Window, &mut App)>;
pub(super) struct ChipPicker {
    query: Entity<InputState>,
    current_query: String,
    core: Arc<chipdock_service::api::ChipDockCore>,
    initial: String,
    results: Vec<String>,
    selected: Option<String>,
    searching: bool,
    revision: u64,
    error: Option<String>,
    on_select: SelectionHandler,
}
impl ChipPicker {
    pub(super) fn new(
        initial: String,
        core: Arc<chipdock_service::api::ChipDockCore>,
        w: &mut Window,
        cx: &mut Context<Self>,
        on_select: impl Fn(Option<String>, &mut Window, &mut App) + 'static,
    ) -> Self {
        let query = cx.new(|cx| InputState::new(w, cx).placeholder("搜索芯片型号，例如 STM32F407"));
        cx.observe(&query, |s, _, cx| s.search(cx)).detach();
        Self {
            query,
            core,
            selected: Some(initial.clone()),
            initial,
            current_query: String::new(),
            results: Vec::new(),
            searching: false,
            revision: 0,
            error: None,
            on_select: Box::new(on_select),
        }
    }
    fn search(&mut self, cx: &mut Context<Self>) {
        let query = self.query.read(cx).value().to_string();
        if query == self.current_query {
            return;
        }
        self.current_query = query.clone();
        self.revision += 1;
        self.error = None;
        self.results.clear();
        if query.trim().is_empty() {
            self.searching = false;
            self.selected = Some(self.initial.clone());
            cx.notify();
            return;
        }
        self.searching = true;
        self.selected = None;
        let revision = self.revision;
        let timer = cx.background_executor().timer(Duration::from_millis(200));
        let core = self.core.clone();
        let task = cx.background_executor().spawn(async move {
            timer.await;
            core.search_chips(query, 150)
        });
        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |s, cx| {
                if s.revision != revision {
                    return;
                }
                s.searching = false;
                match result {
                    Ok(results) => {
                        s.selected = results.first().cloned();
                        s.results = results;
                    }
                    Err(e) => s.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn confirm(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if !self.searching
            && let Some(selection) = self.selected.clone()
        {
            (self.on_select)(Some(selection), w, cx);
        }
    }
}
impl Render for ChipPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let names = if self.current_query.trim().is_empty() {
            let mut v = vec![String::new()];
            if !self.initial.is_empty() {
                v.push(self.initial.clone());
            }
            v
        } else {
            self.results.clone()
        };
        let count = names.iter().filter(|n| !n.is_empty()).count();
        let mut list = div()
            .id("chip-results")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .border_1()
            .border_color(cx.theme().border);
        if names.is_empty() && !self.searching && self.error.is_none() {
            list = list.child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .text_color(cx.theme().muted_foreground)
                    .child(icons::icon("search").size(px(36.)))
                    .child("没有搜索结果")
                    .child(self.current_query.clone()),
            );
        } else {
            for i in 0..names.len().max(13) {
                let name = names.get(i).cloned();
                let mut row = div()
                    .id(("chip-result", i))
                    .h(px(24.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .bg(
                        if name
                            .as_ref()
                            .is_some_and(|n| Some(n) == self.selected.as_ref())
                        {
                            cx.theme().foreground.opacity(0.14)
                        } else if i % 2 == 1 {
                            cx.theme().foreground.opacity(0.04)
                        } else {
                            cx.theme().background
                        },
                    );
                if let Some(name) = name {
                    row = row
                        .child(
                            icons::icon(if name.is_empty() {
                                "auto-detect"
                            } else {
                                "cpu"
                            })
                            .text_color(rgb(0x007aff)),
                        )
                        .child(
                            div()
                                .when(!name.is_empty(), |d| {
                                    d.font_family(cx.theme().mono_font_family.clone())
                                })
                                .child(if name.is_empty() {
                                    "自动识别".to_owned()
                                } else {
                                    name.clone()
                                }),
                        )
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.selected = Some(name.clone());
                            cx.notify();
                        }));
                }
                list = list.child(row);
            }
        }
        div()
            .h(px(480.))
            .p_6()
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, _, cx| {
                let delta = match e.keystroke.key.as_str() {
                    "up" => -1,
                    "down" => 1,
                    _ => return,
                };
                let names = if s.current_query.trim().is_empty() {
                    let mut v = vec![String::new()];
                    if !s.initial.is_empty() {
                        v.push(s.initial.clone());
                    }
                    v
                } else {
                    s.results.clone()
                };
                if !names.is_empty() {
                    let index = s
                        .selected
                        .as_ref()
                        .and_then(|n| names.iter().position(|v| v == n))
                        .unwrap_or(0);
                    s.selected = Some(
                        names[(index as isize + delta).clamp(0, names.len() as isize - 1) as usize]
                            .clone(),
                    );
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .flex()
            .flex_col()
            .gap(px(14.))
            .text_size(px(13.))
            .line_height(relative(1.2))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(div().font_weight(FontWeight::BOLD).child("选择芯片"))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("搜索 probe-rs 支持的芯片型号。"),
                    ),
            )
            .child(
                Input::new(&self.query)
                    .small()
                    .rounded_full()
                    .prefix(icons::icon("search"))
                    .aria_label("搜索芯片型号"),
            )
            .when_some(self.error.clone(), |v, e| {
                v.child(div().text_color(rgb(0xff3b30)).child(e))
            })
            .child(list)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(count > 0, |v| {
                        v.child(
                            div()
                                .text_size(px(11.))
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{count} 个目标")),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        bordered("chip-back", None, "返回")
                            .on_click(cx.listener(|s, _, w, cx| (s.on_select)(None, w, cx))),
                    )
                    .child(
                        bordered("chip-confirm", None, "选择")
                            .primary()
                            .disabled(self.selected.is_none() || self.searching)
                            .on_click(cx.listener(|s, _, w, cx| s.confirm(w, cx))),
                    ),
            )
    }
}
