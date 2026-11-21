//! The session-owned plot window, matching SerialPlotView's controls and geometry.
use super::*;
use nsh_base::plot::{AxisPlacement, Curve, Scale, grid, line, plot, plot_axis};
use nsh_base::{SelectEntry, SelectItem, select};
use nsh_core::element::{RowAccessor, Rows};
use nsh_core::geometry::{Points, Size};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
struct PlotSeries {
    session: u64,
    points: Memo<Vec<Value>>,
    filter: Signal<String>,
    all_names: Memo<Vec<String>>,
    shown_names: Memo<Vec<String>>,
}

pub(super) fn prepare(store: Store, id: u64) {
    let filter = signal(String::new());
    let points = computed(move || {
        store.read(|s| {
            s.sessions
                .get(&id)
                .map(|s| s.points.clone())
                .unwrap_or_default()
        })
    });
    let all_names = computed(move || {
        points.with(|p| {
            p.iter()
                .filter_map(|p| p["values"].as_object())
                .flat_map(|v| v.keys())
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        })
    });
    let shown_names = computed(move || {
        let selected = filter.get();
        all_names.with(|names| {
            names
                .iter()
                .filter(|name| selected.is_empty() || **name == selected)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    provide_context(PlotSeries {
        session: id,
        points,
        filter,
        all_names,
        shown_names,
    });
}

fn series() -> PlotSeries {
    use_context().expect("plot window context")
}

/// Commands and connection title for this plot’s captured serial session.
#[component]
pub fn PlotActions() -> impl IntoElement {
    let store = auxiliary_store();
    let PlotSeries {
        session: id,
        points,
        filter,
        ..
    } = series();
    view()
        .h(px(25.5))
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            text(move || {
                store.read(|s| {
                    s.sessions
                        .get(&id)
                        .and_then(|s| s.status.as_ref())
                        .and_then(|s| s.path.clone())
                        .unwrap_or_else(|| format!("会话 {id}"))
                })
            })
            .text_size(body_size)
            .font_weight(FontWeight::SEMIBOLD),
        )
        .child(view().flex_1())
        .child(
            nsh_base::checkbox()
                .checked(move || store.read(|s| s.sessions.get(&id).is_none_or(|s| !s.plotting)))
                .indicator_class("hidden")
                .disabled(move || store.read(|s| !s.sessions.contains_key(&id) || s.installing))
                .on_change(move |paused: &bool| {
                    store.change(|s| {
                        if let Some(s) = s.sessions.get_mut(&id) {
                            s.plotting = !*paused;
                            s.plot_revision += 1;
                        }
                    })
                })
                .w(px(66.5))
                .h(px(24.))
                .px(px(10.))
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(5.))
                .rounded(move || theme().radius)
                .bg(move || theme().color("plot-control").unwrap_or(theme().secondary))
                .checked_style(|s| {
                    s.bg(move || {
                        theme()
                            .color("plot-control-selected")
                            .unwrap_or(theme().primary)
                    })
                    .text_color(move || theme().primary_foreground)
                })
                .hover(|s| {
                    s.bg(move || {
                        if store.read(|s| s.sessions.get(&id).is_some_and(|s| !s.plotting)) {
                            theme()
                                .color("plot-control-selected")
                                .unwrap_or(theme().primary)
                        } else {
                            theme().accent
                        }
                    })
                })
                .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
                .child(icons::icon("terminal-pause").translate_x(px(-1.5)))
                .child(text("暂停").text_size(body_size).translate_x(px(-2.5))),
        )
        .child(plot_button(
            "trash",
            "清空",
            72.5,
            24.5,
            move || store.read(|s| !s.sessions.contains_key(&id) || s.installing),
            move || {
                store.change(|s| {
                    if let Some(s) = s.sessions.get_mut(&id) {
                        s.points.clear();
                        s.plot_revision += 1;
                        s.plot_cursor = s.cursor;
                        s.plot_dropped = 0;
                    }
                })
            },
        ))
        .child(
            nsh_base::context_menu()
                .disabled(move || points.with(Vec::is_empty))
                .popup_style(popup)
                .trigger(plot_button(
                    "export",
                    "导出 CSV…",
                    111.,
                    25.5,
                    move || points.with(Vec::is_empty),
                    move || {
                        store.export(
                            "ChipDock-Plot.csv",
                            crate::presentation::plot_csv(
                                &points.get_untracked(),
                                &filter.get_untracked(),
                            ),
                        )
                    },
                ))
                .child(menu_item(
                    "导出 JSON…",
                    move || match serde_json::to_vec_pretty(&points.get_untracked()) {
                        Ok(bytes) => store.export("ChipDock-Plot.json", bytes),
                        Err(error) => store.report(error),
                    },
                )),
        )
}

#[derive(Clone, Copy)]
struct PlotDraft {
    mode: Signal<String>,
    fields: Signal<String>,
    template: Signal<String>,
    binary: Signal<String>,
    sample: Signal<String>,
    result: Signal<String>,
    previewing: Signal<bool>,
}
impl PlotDraft {
    fn configuration(self) -> Result<Value, String> {
        configuration(
            &self.mode.get_untracked(),
            &self.fields.get_untracked(),
            &self.template.get_untracked(),
            &self.binary.get_untracked(),
        )
    }
}
fn plot_draft() -> PlotDraft {
    use_context().expect("plot form owns its draft")
}

#[derive(Debug, Clone, Default, PartialEq, JsObject)]
pub struct PlotPresentation {
    /// Show the binary field editor.
    pub binary: bool,
    /// Sample decoding outcome for this draft.
    pub result: String,
    /// Samples outside the retention window.
    pub dropped: u64,
    /// Whether the captured session has no samples.
    pub empty: bool,
    /// Whether this session is collecting.
    pub sampling: bool,
    /// Visible series in native drawing order.
    pub names: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Event)]
pub enum PlotPageEvent {
    /// Visible form state or series membership changed.
    #[args(state)]
    PresentationChange(PlotPresentation),
}
#[derive(Debug, Clone, PartialEq, Props)]
pub struct PlotPageProps {
    /// The page's form, native plot, and legend.
    pub children: Prop<Children>,
}
/// Owns the per-window draft, keyed to its captured serial session.
#[component]
pub fn PlotPage(props: PlotPageProps, cx: Scope<PlotPageEvent>) -> impl IntoElement {
    let store = auxiliary_store();
    let PlotSeries {
        session: id,
        points,
        shown_names,
        ..
    } = series();
    let initial = store.read(|s| {
        let p = if s.active == Some(id) {
            Some(&s.preferences)
        } else {
            s.sessions.get(&id).map(|s| &s.preferences)
        };
        p.and_then(|p| serde_json::from_str::<Value>(p.get("plot")).ok())
            .unwrap_or_else(|| json!({"mode":"numbers"}))
    });
    let mode = signal(initial["mode"].as_str().unwrap_or("numbers").to_owned());
    let fields = signal(
        initial["fields"]
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default(),
    );
    let template = signal(initial["template"].as_str().unwrap_or("").to_owned());
    let binary=signal(initial.get("binary").map(Value::to_string).unwrap_or_else(||r#"[{"name":"value","offset":0,"type":"u16","littleEndian":false,"scale":1,"bias":0}]"#.into()));
    let sample = signal(String::new());
    let result = signal(String::new());
    let previewing = signal(false);

    provide_context(PlotDraft {
        mode,
        fields,
        template,
        binary,
        sample,
        result,
        previewing,
    });
    let presentation = computed(move || {
        let (dropped, sampling) = store.read(|s| {
            s.sessions.get(&id).map_or((0, false), |session| {
                (session.plot_dropped, session.plotting)
            })
        });
        PlotPresentation {
            binary: mode.get() == "binary",
            result: result.get(),
            dropped,
            empty: points.with(Vec::is_empty),
            sampling,
            names: shown_names.get(),
        }
    });
    effect(move || cx.emit(PlotPageEvent::PresentationChange(presentation.get())));
    view().child(props.children)
}

/// Selects the input decoder used by this plot.
#[component]
pub fn PlotModePicker() -> impl IntoElement {
    let mode = plot_draft().mode;
    picker(
        "输入",
        mode,
        vec![
            ("numbers", "数值 / 分隔值"),
            ("kv", "键值对"),
            ("json", "JSON"),
            ("binary", "二进制"),
        ],
    )
}

/// The three single-line editors in the plot configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PropEnum)]
pub enum PlotDraftField {
    /// Comma-separated measurement names.
    #[default]
    Fields,
    /// Optional protocol rule used to decode points.
    Template,
    /// Example payload for the preview operation.
    Sample,
}
#[derive(Debug, Clone, PartialEq, Props)]
pub struct PlotDraftInputProps {
    /// Which editor this field binds in the window's draft.
    pub field: Prop<PlotDraftField>,
}
/// Styled native field bound to the plot window draft.
#[component]
pub fn PlotDraftInput(props: PlotDraftInputProps) -> impl IntoElement {
    let PlotDraft {
        fields,
        template,
        sample,
        mode,
        ..
    } = plot_draft();
    match props.field.get() {
        PlotDraftField::Fields => draft_input(
            "字段名，逗号分隔",
            move || fields.get(),
            move |v: &str| fields.set(v.into()),
        )
        .placeholder("字段名，逗号分隔")
        .disabled(move || mode.get() == "binary"),
        PlotDraftField::Template => draft_input(
            "协议规则名，可选",
            move || template.get(),
            move |v: &str| template.set(v.into()),
        )
        .placeholder("协议规则名，可选"),
        PlotDraftField::Sample => draft_input(
            "样例文本",
            move || sample.get(),
            move |v: &str| sample.set(v.into()),
        )
        .placeholder(move || {
            if mode.get() == "binary" {
                "样例 HEX"
            } else {
                "样例文本"
            }
        }),
    }
}

/// Validates the captured configuration and restarts its sample series.
#[component]
pub fn PlotApply() -> impl IntoElement {
    let store = auxiliary_store();
    let PlotSeries {
        session: id,
        filter,
        ..
    } = series();
    let draft = plot_draft();
    let PlotDraft {
        previewing, result, ..
    } = draft;
    let apply = move || match draft.configuration() {
        Ok(config) => {
            previewing.set(true);
            spawn(async move {
                let validation = json!({"action":"plot","config":config,"sample":""}).to_string();
                let checked = nsh_core::element::blocking(move || {
                    chipdock_service::serial::serial_workbench(id, &validation)
                })
                .await;
                previewing.set(false);
                match checked {
                    Ok(Ok(_)) => {
                        let source = config.to_string();
                        store.change(|s| {
                            if s.installing {
                                return;
                            }
                            let active = s.active == Some(id);
                            if let Some(session) = s.sessions.get_mut(&id) {
                                session
                                    .preferences
                                    .fields
                                    .insert("plot".into(), source.clone());
                                session.plot_revision += 1;
                                session.points.clear();
                                session.plot_dropped = 0;
                                if active {
                                    s.preferences.fields.insert("plot".into(), source);
                                }
                            }
                        });
                        filter.set(String::new());
                        result.set(String::new());
                    }
                    Ok(Err(error)) => result.set(error.to_string()),
                    Err(error) => result.set(error.to_string()),
                }
            });
        }
        Err(error) => result.set(error),
    };
    plot_button(
        "",
        "应用",
        50.,
        24.,
        move || previewing.get() || store.read(|s| s.installing || !s.sessions.contains_key(&id)),
        apply,
    )
}

/// Multiline binary layout JSON editor with measured height.
#[component]
pub fn PlotBinaryEditor() -> impl IntoElement {
    let binary = plot_draft().binary;
    let binary_height = signal(35.5_f32);
    nsh_base::input()
        .multiline(true)
        .value(move || binary.get())
        .on_change(move |v: &str| binary.set(v.into()))
        .on_content_size_change(move |size: &Size<Points>| {
            binary_height.set((size.height.0 + 8.).clamp(35.5, 60.))
        })
        .label("字段配置 JSON")
        .h(move || px(binary_height.get()))
        .px(px(6.))
        .py_1()
        .rounded(move || theme().radius)
        .outline(px(1.))
        .outline_color(move || theme().color("plot-input-outline").unwrap_or(theme().input))
        .bg(move || theme().color("input-well").unwrap_or(theme().background))
        .text_color(move || theme().foreground)
        .caret_color(|| color(theme().primary))
        .focus(|s| {
            s.outline(px(3.))
                .outline_color(move || theme().color("input-focus-ring").unwrap_or(theme().ring))
        })
        .font_family(mono)
        .text_size(px(10.))
        .line_height(px(13.))
}

/// Validates and decodes one sample against the active configuration draft.
#[component]
pub fn PlotPreview() -> impl IntoElement {
    let id = series().session;
    let draft = plot_draft();
    let PlotDraft {
        sample,
        mode,
        previewing,
        result,
        ..
    } = draft;
    plot_button(
        "",
        "试算",
        50.,
        24.,
        move || sample.with(String::is_empty) || previewing.get(),
        move || match draft.configuration() {
            Ok(config) => {
                let text = sample.get_untracked();
                let hex = mode.get_untracked() == "binary";
                previewing.set(true);
                spawn(async move {
                    let request = json!({"action":"plot","config":config,"sample":text,"hex":hex})
                        .to_string();
                    let answer = nsh_core::element::blocking(move || {
                        chipdock_service::serial::serial_workbench(id, &request)
                    })
                    .await;
                    previewing.set(false);
                    result.set(match answer {
                        Ok(Ok(value)) => value,
                        Ok(Err(e)) => e.to_string(),
                        Err(e) => e.to_string(),
                    });
                });
            }
            Err(error) => result.set(error),
        },
    )
}

/// Selects one visible field or all measured fields.
#[component]
pub fn PlotFieldPicker() -> impl IntoElement {
    let PlotSeries {
        filter, all_names, ..
    } = series();
    field_picker(filter, all_names)
}

/// Native plot with numeric series, bounded sampling and device-sequence axes.
#[component]
pub fn PlotTrace() -> impl IntoElement {
    let PlotSeries {
        points,
        shown_names,
        ..
    } = series();
    let rows = computed(move || {
        points.with(|p| {
            Rows::from_columns(
                p.len(),
                shown_names.with(|names| {
                    names
                        .iter()
                        .map(|name| {
                            (
                                name.clone().into(),
                                p.iter()
                                    .map(|p| p["values"][name].as_f64().unwrap_or(f64::NAN))
                                    .collect(),
                            )
                        })
                        .collect::<Vec<_>>()
                }),
            )
        })
    });
    let sequence_axis = computed(move || points.with(|p| plot_sequence_axis(p)));
    plot()
        .data(rows)
        .x(move || Some(sequence_axis.get()))
        .x_scale(Scale::Linear {
            domain: None,
            reverse: false,
        })
        .flex_1()
        .ml(px(44.))
        .mr_2()
        .mt_2()
        .mb_6()
        .accessibility_label("串口测量曲线")
        .child(
            grid()
                .stroke_color(move || theme().border.opacity(0.65))
                .stroke_width(px(1.)),
        )
        .child(
            plot_axis()
                .placement(AxisPlacement::Bottom)
                .ticks(6_u32)
                .label_class("text-xs text-muted-foreground pt-1")
                .tick_class("w-px h-1 bg-border")
                .line_class("h-px bg-border"),
        )
        .child(
            plot_axis()
                .placement(AxisPlacement::Left)
                .ticks(5_u32)
                .label_class("text-xs text-muted-foreground pr-1")
                .tick_class("w-1 h-px bg-border")
                .line_class("w-px bg-border"),
        )
        .child(each(
            shown_names,
            |name: &String| name.clone(),
            move |name, index| {
                line()
                    .y(move || Some(RowAccessor::key(name.get())))
                    .curve(Curve::Linear)
                    .stroke_width(px(2.))
                    .text_color(move || match index.get() % 5 {
                        0 => theme().chart_1,
                        1 => theme().chart_2,
                        2 => theme().chart_3,
                        3 => theme().chart_4,
                        _ => theme().chart_5,
                    })
            },
        ))
}

fn plot_sequence_axis(points: &[Value]) -> RowAccessor {
    let sequences = points
        .iter()
        .map(|point| point["seq"].as_f64().unwrap_or(0.))
        .collect::<Vec<_>>();
    RowAccessor::from_fn(move |index| sequences.get(index).copied().unwrap_or(f64::NAN))
}

fn picker(
    label: &'static str,
    value: Signal<String>,
    choices: Vec<(&'static str, &'static str)>,
) -> View {
    let titles = choices.clone();
    view()
        .flex_row()
        .items_center()
        .justify_center()
        .gap_2()
        .child(text(label).text_size(body_size))
        .child(
            select()
                .w(px(123.5))
                .accessibility_label(label)
                .items(
                    choices
                        .into_iter()
                        .map(|(v, t)| SelectItem::new(v, t))
                        .collect::<Vec<_>>(),
                )
                .value(move || Some(SharedString::from(value.get())))
                .on_change(move |v: &Option<SharedString>| {
                    if let Some(v) = v {
                        value.set(v.to_string())
                    }
                })
                .popup_style(|s| {
                    popup(s)
                        .p(px(5.))
                        .w(px(124.))
                        .translate_x(px(-12.5))
                        .translate_y(px(0.5))
                })
                .item_class("focus:bg-plot-menu-highlight focus:text-primary-foreground")
                .item_style(|s| s.rounded(px(8.)))
                .render_item(move |item: &SelectEntry| {
                    let key = item.value.clone();
                    picker_item(item, move || value.with(|v| v.as_str() == key.as_ref()))
                })
                .trigger(
                    view()
                        .h(px(24.))
                        .flex_row()
                        .items_center()
                        .pl(px(12.))
                        .pr_2()
                        .gap(px(6.))
                        .rounded(px(7.))
                        .bg(move || theme().color("plot-control").unwrap_or(theme().secondary))
                        .child(
                            text(move || {
                                let current = value.get();
                                titles
                                    .iter()
                                    .find(|(v, _)| *v == current)
                                    .map_or(current.clone(), |(_, t)| (*t).into())
                            })
                            .text_size(body_size)
                            .flex_1(),
                        )
                        .child(icons::icon("plot-select").size(px(12.))),
                ),
        )
}
fn field_picker(filter: Signal<String>, names: Memo<Vec<String>>) -> View {
    view()
        .flex_row()
        .items_center()
        .justify_center()
        .gap_2()
        .child(text("显示字段").text_size(body_size))
        .child(
            select()
                .w(px(73.5))
                .accessibility_label("显示字段")
                .items(move || {
                    std::iter::once(SelectItem::new("", "全部"))
                        .chain(
                            names
                                .get()
                                .into_iter()
                                .map(|name| SelectItem::new(name.clone(), name)),
                        )
                        .collect::<Vec<_>>()
                })
                .value(move || Some(SharedString::from(filter.get())))
                .on_change(move |v: &Option<SharedString>| {
                    if let Some(v) = v {
                        filter.set(v.to_string())
                    }
                })
                .popup_style(|s| {
                    popup(s)
                        .p(px(5.))
                        .w(px(74.))
                        .translate_x(px(-13.))
                        .translate_y(px(0.5))
                })
                .item_class("focus:bg-plot-menu-highlight focus:text-primary-foreground")
                .item_style(|s| s.rounded(px(8.)))
                .render_item(move |item: &SelectEntry| {
                    let key = item.value.clone();
                    picker_item(item, move || filter.with(|v| v.as_str() == key.as_ref()))
                })
                .trigger(
                    view()
                        .h(px(24.))
                        .pl(px(12.))
                        .pr(px(10.))
                        .flex_row()
                        .items_center()
                        .gap(px(6.))
                        .rounded(px(7.))
                        .bg(move || theme().color("plot-control").unwrap_or(theme().secondary))
                        .child(
                            text(move || {
                                let f = filter.get();
                                if f.is_empty() { "全部".into() } else { f }
                            })
                            .text_size(body_size)
                            .flex_1(),
                        )
                        .child(icons::icon("plot-select").size(px(12.)).translate_x(px(2.))),
                ),
        )
}
fn picker_item(item: &SelectEntry, selected: impl Fn() -> bool + 'static) -> View {
    view()
        .h(px(24.))
        .pl(px(7.))
        .pr_2()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .child(
            view()
                .size(px(12.))
                .child(when(selected, || icons::icon("check").size(px(12.)))),
        )
        .child(text(item.title.clone()).text_size(body_size))
}
fn configuration(mode: &str, fields: &str, template: &str, binary: &str) -> Result<Value, String> {
    let mut value = json!({"mode":mode,"template":template});
    if mode == "binary" {
        value["binary"] =
            serde_json::from_str(binary).map_err(|e| format!("字段配置 JSON 无效：{e}"))?;
    } else {
        value["fields"] = json!(
            fields
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        );
    }
    Ok(value)
}

fn plot_button(
    icon: &'static str,
    label: &'static str,
    width: f32,
    height: f32,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    let button = nsh_base::button()
        .on_press(press)
        .disabled(disabled)
        .w(px(width))
        .h(px(height))
        .px_3()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(5.))
        .rounded(move || theme().radius)
        .bg(move || theme().color("plot-control").unwrap_or(theme().secondary))
        .text_color(move || theme().foreground)
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().muted))
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .disabled_style(|s| {
            s.bg(move || {
                theme()
                    .color("plot-control-disabled")
                    .unwrap_or(theme().muted)
            })
            .text_color(move || {
                theme()
                    .color("plot-control-disabled-foreground")
                    .or_else(|| theme().color("button-disabled-foreground"))
                    .unwrap_or(theme().muted_foreground)
            })
        });
    let button = if icon.is_empty() {
        button
    } else {
        let image = icons::icon(icon);
        let image = match icon {
            "trash" => image
                .scale_x(25. / 26.)
                .scale_y(29. / 28.)
                .translate_x(px(-1.75))
                .translate_y(px(-0.25)),
            "export" => image
                .scale_x(23. / 22.)
                .scale_y(29. / 28.)
                .translate_x(px(-1.75))
                .translate_y(px(0.75)),
            _ => image,
        };
        button.child(image)
    };
    button.child(
        text(label)
            .text_size(body_size)
            .translate_x(px(if icon.is_empty() { 0. } else { 0.5 })),
    )
}

pub(super) fn register(registry: &mut Registry) {
    registry
        .register::<PlotActions>()
        .register::<PlotTrace>()
        .register::<PlotPage>()
        .register::<PlotModePicker>()
        .register::<PlotDraftInput>()
        .register::<PlotApply>()
        .register::<PlotBinaryEditor>()
        .register::<PlotPreview>()
        .register::<PlotFieldPicker>();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plot_axis_preserves_sparse_core_sequences() {
        let points = [json!({"seq":1}), json!({"seq":5}), json!({"seq":50})];
        let rows = Rows::from_columns(3, [("value".into(), vec![1., 2., 3.])]);
        let values = plot_sequence_axis(&points).values_now(&rows).unwrap();
        assert_eq!(values.as_ref(), &[1., 5., 50.]);

        let retained = Rows::from_columns(2, [("value".into(), vec![2., 3.])]);
        let values = plot_sequence_axis(&points[1..])
            .values_now(&retained)
            .unwrap();
        assert_eq!(values.as_ref(), &[5., 50.]);
    }
    #[test]
    fn configuration_preserves_native_input_modes() {
        for mode in ["numbers", "kv", "json"] {
            let c = configuration(mode, " temperature, voltage ", "rx", "").unwrap();
            assert_eq!(c["fields"], json!(["temperature", "voltage"]));
            assert_eq!(c["template"], "rx");
        }
    }
    #[test]
    fn binary_json_is_parsed_before_dispatch() {
        assert_eq!(
            configuration("binary", "", "", "[]").unwrap()["binary"],
            json!([])
        );
        assert!(configuration("binary", "", "", "not json").is_err());
    }
}
