//! Structured color-rule sheet, matching the native workbench.
use super::*;
use nsh_base::{SelectEntry, SelectItem};
use serde::{Deserialize, Serialize};

const PALETTE: [(&str, &str); 5] = [
    ("red", "红色"),
    ("orange", "橙色"),
    ("green", "绿色"),
    ("blue", "蓝色"),
    ("purple", "紫色"),
];

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Rule {
    #[serde(skip)]
    id: u64,
    pattern: String,
    #[serde(default)]
    regex: bool,
    color: String,
}

pub(super) fn page(store: Store) -> View {
    let captured = store.tool_session.get_untracked();
    let initial = match serde_json::from_str::<Vec<Rule>>(&store.field("colors")) {
        Ok(rules) => rules,
        Err(error) => {
            return view()
                .w(px(620.))
                .h(px(420.))
                .p_5()
                .gap_3()
                .child(
                    text(format!("着色规则无效：{error}"))
                        .text_size(body_size)
                        .text_color(move || theme().destructive),
                )
                .child(view().flex_1())
                .child(action(
                    "完成",
                    Variant::Default,
                    || false,
                    move || tools::close(store),
                ));
        }
    };
    let rows = signal(
        initial
            .into_iter()
            .enumerate()
            .map(|(i, mut rule)| {
                rule.id = i as u64;
                rule
            })
            .collect::<Vec<_>>(),
    );
    let next = stored(rows.with_untracked(|r| r.len() as u64));
    let tx = signal(store.field("tx_color"));
    let failure = signal(String::new());
    let valid_session = move || store.read(|s| s.active == captured && !s.installing);

    view()
        .w(px(620.))
        .h(px(420.))
        .p_5()
        .gap_3()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(native_menu(
                    view()
                        .w(px(73.5))
                        .h(px(24.))
                        .px(px(12.))
                        .rounded(px(7.))
                        .bg(move || theme().color("colors-control").unwrap_or(theme().secondary))
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .label("着色预设")
                        .child(text("预设").text_size(body_size))
                        .child(view().flex_1())
                        .child(icons::icon("down").size(px(10.))),
                    move || {
                        ["ESP-IDF", "日志等级", "AT 响应"]
                            .into_iter()
                            .map(move |name| {
                                nsh_core::element::desktop::MenuItem::new(name).on_select(
                                    move || {
                                        rows.set(preset(name));
                                        next.replace(rows.with_untracked(|r| r.len() as u64));
                                        failure.set(String::new());
                                    },
                                )
                            })
                            .collect()
                    },
                ))
                .child(color_action(
                    "添加规则",
                    Some("plus"),
                    97.5,
                    move || rows.with(|r| r.len() >= 32),
                    move || {
                        let id = next.with(|n| {
                            let id = *n;
                            *n += 1;
                            id
                        });
                        rows.update(|r| {
                            r.push(Rule {
                                id,
                                pattern: String::new(),
                                regex: false,
                                color: "orange".into(),
                            })
                        });
                    },
                ))
                .child(view().flex_1())
                .child(text("TX 颜色").text_size(body_size))
                .child(picker(move || tx.get(), move |v| tx.set(v), true)),
        )
        .child(
            view()
                .relative()
                .flex_1()
                .min_h_0()
                .border_1()
                .border_color(hairline)
                .child(
                    virtual_list(
                        move || rows.with(Vec::len),
                        move |at| {
                            let current =
                                computed(move || rows.with(|r| r.get(at.index()).cloned()));
                            view()
                                .h(px(32.))
                                .px(px(6.))
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .bg(move || {
                                    if at.index() % 2 == 1 {
                                        theme().color("table-stripe").unwrap_or(theme().muted)
                                    } else {
                                        theme().background
                                    }
                                })
                                .child(
                                    draft_input(
                                        "匹配关键字",
                                        move || {
                                            current.with(|r| {
                                                r.as_ref()
                                                    .map(|r| r.pattern.clone())
                                                    .unwrap_or_default()
                                            })
                                        },
                                        move |v: &str| {
                                            rows.update(|r| {
                                                if let Some(row) = r.get_mut(at.index()) {
                                                    row.pattern = v.into();
                                                }
                                            })
                                        },
                                    )
                                    .placeholder("匹配关键字")
                                    .flex_1()
                                    .min_w_0(),
                                )
                                .child(draft_toggle(
                                    "正则",
                                    move || current.with(|r| r.as_ref().is_some_and(|r| r.regex)),
                                    move |v| {
                                        rows.update(|r| {
                                            if let Some(row) = r.get_mut(at.index()) {
                                                row.regex = v;
                                            }
                                        })
                                    },
                                ))
                                .child(picker(
                                    move || {
                                        current.with(|r| {
                                            r.as_ref().map(|r| r.color.clone()).unwrap_or_default()
                                        })
                                    },
                                    move |v| {
                                        rows.update(|r| {
                                            if let Some(row) = r.get_mut(at.index()) {
                                                row.color = v;
                                            }
                                        })
                                    },
                                    false,
                                ))
                                .child(row_button(
                                    "up",
                                    "上移",
                                    14.,
                                    move || at.index() == 0,
                                    move || {
                                        rows.update(|r| {
                                            let i = at.index();
                                            if i > 0 && i < r.len() {
                                                r.swap(i, i - 1);
                                            }
                                        })
                                    },
                                ))
                                .child(row_button(
                                    "trash",
                                    "删除",
                                    16.,
                                    || false,
                                    move || {
                                        rows.update(|r| {
                                            let i = at.index();
                                            if i < r.len() {
                                                r.remove(i);
                                            }
                                        })
                                    },
                                ))
                        },
                    )
                    .keys(move || rows.with(|r| r.iter().map(|r| r.id).collect::<Vec<_>>()))
                    .estimated_item_size(px(32.))
                    .flex_1(),
                )
                // Native List continues its 24pt alternating empty rows below its data.
                .child(
                    view()
                        .absolute()
                        .inset_0()
                        .overflow_hidden()
                        .pointer_events(nsh_core::PointerEvents::None)
                        .accessibility_elements_hidden(true)
                        .child(
                            view()
                                .h(move || px(rows.with(Vec::len) as f32 * 32.))
                                .flex_shrink_0(),
                        )
                        .children((0..14).map(move |index| {
                            view().h(px(24.)).flex_shrink_0().bg(move || {
                                if (rows.with(Vec::len) + index) % 2 == 1 {
                                    theme().color("table-stripe").unwrap_or(theme().muted)
                                } else {
                                    theme().background
                                }
                            })
                        })),
                )
                .child(when(
                    move || rows.with(Vec::is_empty),
                    || {
                        view()
                            .absolute()
                            .inset_0()
                            .items_center()
                            .justify_center()
                            .child(
                                // Measured native ContentUnavailableView: 44pt symbol,
                                // 18.25pt label gap, two 19pt lines, 6pt description gap.
                                view()
                                    .h(px(121.))
                                    .items_center()
                                    .child(
                                        icons::icon("palette")
                                            .size(px(44.))
                                            .mb(px(18.25))
                                            .text_color(move || {
                                                theme()
                                                    .color("empty-symbol")
                                                    .unwrap_or(theme().muted_foreground)
                                            }),
                                    )
                                    .child(
                                        text("没有着色规则")
                                            .text_size(px(15.))
                                            .line_height(px(19.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(move || {
                                                theme()
                                                    .color("empty-foreground")
                                                    .unwrap_or(theme().muted_foreground)
                                            }),
                                    )
                                    .child(
                                        text("接收数据匹配关键字时按指定颜色显示。")
                                            .mt(px(6.))
                                            .text_size(px(15.))
                                            .line_height(px(19.))
                                            .text_color(move || {
                                                theme()
                                                    .color("empty-foreground")
                                                    .unwrap_or(theme().muted_foreground)
                                            }),
                                    ),
                            )
                    },
                )),
        )
        .child(when(
            move || !failure.get().is_empty(),
            move || {
                view()
                    .flex_row()
                    .items_start()
                    .gap_2()
                    .child(icons::icon("warning-fill").text_color(move || theme().destructive))
                    .child(
                        text(move || failure.get())
                            .text_size(body_size)
                            .text_color(move || theme().destructive),
                    )
            },
        ))
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_end()
                .gap_2()
                .child(color_action(
                    "取消",
                    None,
                    50.,
                    || false,
                    move || tools::close(store),
                ))
                .child(action(
                    "保存",
                    Variant::DefaultAction,
                    move || !valid_session(),
                    move || {
                        let draft = rows.get_untracked();
                        if let Some(message) = draft_error(&draft) {
                            failure.set(message.into());
                            return;
                        }
                        let source = match serde_json::to_string(&draft) {
                            Ok(source) => source,
                            Err(e) => {
                                failure.set(e.to_string());
                                return;
                            }
                        };
                        let tx_value = tx.get_untracked();
                        if let Err(e) = crate::presentation::Colors::new(&source, &tx_value) {
                            failure.set(e.to_string());
                            return;
                        }
                        let result = store.change(|s| -> anyhow::Result<()> {
                            anyhow::ensure!(s.active == captured, "会话已切换，请重新打开着色规则");
                            anyhow::ensure!(!s.installing, "正在安装更新，请稍后保存");
                            let previous = s.preferences.clone();
                            s.set("colors", source);
                            s.set("tx_color", tx_value);
                            if let Err(error) = s.save() {
                                s.preferences = previous.clone();
                                if let Some(session) =
                                    captured.and_then(|id| s.sessions.get_mut(&id))
                                {
                                    session.preferences = previous;
                                }
                                return Err(error);
                            }
                            Ok(())
                        });
                        match result {
                            Ok(()) => tools::close(store),
                            Err(e) => failure.set(format!("{e:#}")),
                        }
                    },
                )),
        )
}

fn draft_error(rules: &[Rule]) -> Option<&'static str> {
    rules
        .iter()
        .any(|rule| {
            rule.pattern.is_empty()
                || rule.pattern.encode_utf16().count() > 256
                || (rule.regex && regex::Regex::new(&rule.pattern).is_err())
        })
        .then_some("规则需要 1–256 字符的关键字或有效正则")
}

fn color_action(
    label: &'static str,
    icon: Option<&'static str>,
    width: f32,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    nsh_base::button()
        .accessibility_label(label)
        .on_press(press)
        .disabled(disabled)
        .w(px(width))
        .h(px(24.))
        .p_0()
        .flex_row()
        .gap(px(5.))
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .bg(move || theme().color("colors-control").unwrap_or(theme().secondary))
        .hover(|s| s.bg(move || theme().accent))
        .disabled_style(|s| s.opacity(0.35))
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .child(when(
            move || icon.is_some(),
            move || icons::icon(icon.unwrap_or("plus")),
        ))
        .child(text(label).text_size(body_size))
}

fn row_button(
    icon: &'static str,
    label: &'static str,
    width: f32,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    nsh_base::button()
        .accessibility_label(label)
        .on_press(press)
        .disabled(disabled)
        .w(px(width))
        .h(px(20.))
        .p_0()
        .items_center()
        .justify_center()
        .text_color(move || theme().foreground.opacity(0.55))
        .disabled_style(|s| s.opacity(0.35))
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .child(icons::icon(icon).size(px(width)))
}

fn picker(
    read: impl Fn() -> String + 'static,
    write: impl Fn(String) + 'static,
    default: bool,
) -> impl IntoElement {
    let current = computed(read);
    let label = if default { "TX 颜色" } else { "规则颜色" };
    nsh_base::select()
        .items(move || {
            let mut items = Vec::new();
            if default {
                items.push(SelectItem::new("default", "默认"));
            }
            items.extend(
                PALETTE
                    .into_iter()
                    .map(|(id, title)| SelectItem::new(id, title)),
            );
            let value = current.get();
            if !value.is_empty() && value != "default" && !PALETTE.iter().any(|p| p.0 == value) {
                items.push(SelectItem::new(value.clone(), value));
            }
            items
        })
        .value(move || {
            Some(SharedString::from(if current.get().is_empty() {
                "default".into()
            } else {
                current.get()
            }))
        })
        .on_change(move |value: &Option<SharedString>| {
            if let Some(value) = value {
                write(value.to_string());
            }
        })
        .accessibility_label(label)
        // Native selected-row bounds, measured in both appearances: 81pt wide;
        // pop-up buttons reserve a different leading inset from borderless row pickers.
        .popup_style(move |style| {
            popup(style).w(px(91.)).px(px(5.))
                .translate_x(px(if default { -12.5 } else { -20. }))
        })
        .item_class("group focus:bg-primary focus:text-primary-foreground selected:font-medium disabled:opacity-50")
        .item_style(|style| style.rounded(px(7.)))
        .render_item(move |entry: &SelectEntry| {
            let value = entry.value.clone();
            let has_swatch = !value.is_empty() && value.as_ref() != "default";
            view().h(px(24.)).pl(px(8.)).pr(px(12.)).flex_row().items_center().gap(px(6.))
                .child(view().w(px(12.)).accessibility_elements_hidden(true).items_center().child(
                    text(move || if current.get().as_str() == value.as_ref() { "✓" } else { "" })
                        .text_size(body_size)))
                .child(view().size(px(13.)).accessibility_elements_hidden(true).items_center().justify_center().child(when(move || has_swatch, || {
                    svg(nsh_core::element::ImageSrc::svg_markup(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="currentColor"/></svg>"#)).size(px(13.))
                })))
                .child(text(entry.title.clone()).text_size(body_size))
        })
        .trigger(
            view()
                .w(px(if default { 90.5 } else { 65. }))
                .h(px(if default { 24. } else { 16. }))
                .px(px(if default { 12. } else { 0. }))
                .rounded(px(7.))
                .when(default, |view| {
                    view.bg(move || theme().color("colors-control").unwrap_or(theme().secondary))
                })
                .flex_row()
                .items_center()
                .gap_1()
                .child(swatch(
                    move || current.get(),
                    move || palette_title(&current.get()),
                    true,
                ))
                .child(view().flex_1())
                .child(icons::icon("select").size(px(12.))),
        )
}

fn swatch(
    value: impl IntoReactive<SharedString>,
    title: impl IntoReactive<SharedString>,
    template: bool,
) -> View {
    let source = value.into_reactive();
    let value = computed(move || source.get());
    view()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .child(when(
            move || !value.get().is_empty() && value.get().as_ref() != "default",
            move || {
                view().size(px(13.)).rounded_full().bg(move || {
                    if template {
                        return theme().foreground;
                    }
                    crate::presentation::color(value.get().as_ref())
                        .ok()
                        .flatten()
                        .map(|c| nsh_core::color::rgb(c).into())
                        .unwrap_or(theme().muted_foreground)
                })
            },
        ))
        .child(text(title).text_size(body_size))
}

fn palette_title(value: &str) -> SharedString {
    PALETTE
        .iter()
        .find(|p| p.0 == value)
        .map(|p| p.1.to_owned())
        .unwrap_or_else(|| {
            if value.is_empty() || value == "default" {
                "默认".into()
            } else {
                value.into()
            }
        })
        .into()
}

fn preset(name: &str) -> Vec<Rule> {
    let values: &[(&str, bool, &str)] = match name {
        "ESP-IDF" => &[("^[EWe] \\(", true, "orange"), ("^[IDV] \\(", true, "blue")],
        "日志等级" => &[
            ("ERROR|FATAL", true, "red"),
            ("WARN", false, "orange"),
            ("INFO", false, "blue"),
        ],
        _ => &[("ERROR", false, "red"), ("OK", false, "green")],
    };
    values
        .iter()
        .enumerate()
        .map(|(id, (pattern, regex, color))| Rule {
            id: id as u64,
            pattern: (*pattern).into(),
            regex: *regex,
            color: (*color).into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_validate_and_draft_ids_never_enter_saved_rules() {
        for name in ["ESP-IDF", "日志等级", "AT 响应"] {
            let source = serde_json::to_string(&preset(name)).unwrap();
            assert!(!source.contains("\"id\""));
            assert!(crate::presentation::Colors::new(&source, "default").is_ok());
        }
    }
    #[test]
    fn invalid_drafts_use_the_native_validation_message() {
        let rule = |pattern: String, regex| Rule {
            id: 0,
            pattern,
            regex,
            color: "orange".into(),
        };
        let expected = Some("规则需要 1–256 字符的关键字或有效正则");
        for invalid in [
            rule(String::new(), false),
            rule("😀".repeat(129), false),
            rule("[".into(), true),
        ] {
            assert_eq!(draft_error(&[invalid]), expected);
        }
        assert_eq!(draft_error(&[rule("😀".repeat(128), false)]), None);
        assert_eq!(draft_error(&[rule("[".into(), false)]), None);
    }
    #[test]
    fn malformed_colors_keep_the_shared_validator_error() {
        let rules = vec![Rule {
            id: 0,
            pattern: "OK".into(),
            regex: false,
            color: "unknown-color".into(),
        }];
        assert_eq!(draft_error(&rules), None);
        assert!(
            crate::presentation::Colors::new(&serde_json::to_string(&rules).unwrap(), "default")
                .is_err()
        );
    }
}
