use super::*;
pub(super) fn surface(series: BTreeMap<String, Vec<(f64, f64)>>, cx: &App) -> AnyElement {
    let colors = [0x007aff, 0xff9500, 0x34c759, 0xaf52de, 0xff3b30, 0x5ac8fa];
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for values in series.values() {
        for (x, y) in values {
            if x.is_finite() && y.is_finite() {
                xs.push(*x);
                ys.push(*y);
            }
        }
    }
    let minx = xs.iter().copied().reduce(f64::min).unwrap_or(0.);
    let maxx = xs.iter().copied().reduce(f64::max).unwrap_or(1.);
    let miny = ys.iter().copied().reduce(f64::min).unwrap_or(0.);
    let maxy = ys.iter().copied().reduce(f64::max).unwrap_or(1.);
    let dx = (maxx - minx).max(1.);
    let dy = (maxy - miny).max(1.);
    let grid = cx.theme().border;
    let legend = div()
        .flex()
        .justify_center()
        .gap_4()
        .children(series.keys().enumerate().map(|(i, name)| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .text_size(px(11.))
                .child(div().w(px(12.)).h(px(2.)).bg(rgb(colors[i % colors.len()])))
                .child(name.clone())
        }));
    div()
        .size_full()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(
                    div()
                        .w(px(48.))
                        .flex()
                        .flex_col()
                        .justify_between()
                        .text_size(px(10.))
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{maxy:.2}"))
                        .child(format!("{:.2}", (miny + maxy) / 2.))
                        .child(format!("{miny:.2}")),
                )
                .child(
                    canvas(
                        move |_, _, _| (),
                        move |bounds, _, window, _| {
                            for row in 0..5 {
                                let y = bounds.top() + bounds.size.height * (row as f32 / 4.);
                                let mut line = PathBuilder::stroke(px(0.5));
                                line.move_to(point(bounds.left(), y));
                                line.line_to(point(bounds.right(), y));
                                if let Ok(path) = line.build() {
                                    window.paint_path(path, grid);
                                }
                            }
                            for (i, values) in series.iter().enumerate() {
                                let mut line = PathBuilder::stroke(px(1.5));
                                let mut first = true;
                                for (x, y) in values.1 {
                                    if !x.is_finite() || !y.is_finite() {
                                        continue;
                                    }
                                    let p = point(
                                        bounds.left()
                                            + bounds.size.width * ((x - minx) / dx) as f32,
                                        bounds.bottom()
                                            - bounds.size.height * ((y - miny) / dy) as f32,
                                    );
                                    if first {
                                        line.move_to(p);
                                        first = false;
                                    } else {
                                        line.line_to(p);
                                    }
                                }
                                if let Ok(path) = line.build() {
                                    window.paint_path(path, rgb(colors[i % colors.len()]));
                                }
                            }
                        },
                    )
                    .flex_1()
                    .h_full(),
                ),
        )
        .child(
            div()
                .pl(px(48.))
                .flex()
                .justify_between()
                .text_size(px(10.))
                .text_color(cx.theme().muted_foreground)
                .child(format!("{minx:.0}"))
                .child("样本")
                .child(format!("{maxx:.0}")),
        )
        .child(legend)
        .into_any_element()
}
