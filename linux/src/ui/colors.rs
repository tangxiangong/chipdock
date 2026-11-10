use super::*;
struct ColorRow {
    id: usize,
    pattern: Entity<InputState>,
    regex: bool,
    color: String,
}
struct ColorEditor {
    owner: WeakEntity<Workbench>,
    session: Option<u64>,
    rows: Vec<ColorRow>,
    next: usize,
    tx: String,
    error: String,
}
const PALETTE: [(&str, &str, u32); 5] = [
    ("red", "红色", 0xff3b30),
    ("orange", "橙色", 0xff9500),
    ("green", "绿色", 0x34c759),
    ("blue", "蓝色", 0x007aff),
    ("purple", "紫色", 0xaf52de),
];
impl Workbench {
    pub(super) fn open_colors(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let source: Vec<Value> =
            match serde_json::from_str(self.editors["colors"].read(cx).value().as_ref()) {
                Ok(v) => v,
                Err(e) => {
                    self.report_error(e.to_string(), cx);
                    return;
                }
            };
        let owner = cx.entity().downgrade();
        let session = self.active;
        let tx = self.fields["tx_color"].read(cx).value().to_string();
        let editor = cx.new(|cx| {
            let mut s = ColorEditor {
                owner,
                session,
                rows: Vec::new(),
                next: 0,
                tx,
                error: String::new(),
            };
            for row in source {
                s.add(
                    row["pattern"].as_str().unwrap_or(""),
                    row["regex"].as_bool().unwrap_or(false),
                    row["color"].as_str().unwrap_or("orange"),
                    w,
                    cx,
                );
            }
            s
        });
        if !w.has_active_dialog(cx) {
            self.focus_handle.focus(w, cx);
        }
        w.open_dialog(cx, move |d, _, _| {
            let saving = editor.clone();
            d.on_ok(move |_, w, cx| {
                saving.update(cx, |editor, cx| editor.save(w, cx));
                false
            })
            .w(px(620.))
            .p_0()
            .close_button(false)
            .child(editor.clone())
        });
    }
}
impl ColorEditor {
    fn add(
        &mut self,
        pattern: &str,
        regex: bool,
        color: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let field = cx.new(|cx| {
            InputState::new(w, cx)
                .placeholder("匹配关键字")
                .default_value(pattern.to_owned())
        });
        cx.observe(&field, |_, _, cx| cx.notify()).detach();
        self.rows.push(ColorRow {
            id: self.next,
            pattern: field,
            regex,
            color: color.into(),
        });
        self.next += 1;
        cx.notify();
    }
    fn save(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let value=json!(self.rows.iter().map(|r|json!({"pattern":r.pattern.read(cx).value().to_string(),"regex":r.regex,"color":r.color})).collect::<Vec<_>>()).to_string();
        let result = (|| {
            chipdock_gpui::presentation::Colors::new(&value, &self.tx)?;
            self.owner.update(cx, |o, cx| -> Result<()> {
                anyhow::ensure!(o.active == self.session, "会话已切换，请重新打开着色规则");
                let mut p = o.snapshot(cx);
                p.fields.insert("colors".into(), value.clone());
                p.fields.insert("tx_color".into(), self.tx.clone());
                p.save(o.path.as_ref().context("设置文件不可用")?)?;
                o.preferences = p;
                o.set("colors", value, w, cx);
                o.set("tx_color", self.tx.clone(), w, cx);
                o.render_terminal(w, cx);
                Ok(())
            })??;
            Ok::<_, anyhow::Error>(())
        })();
        match result {
            Ok(()) => w.close_dialog(cx),
            Err(e) => {
                self.error = e.to_string();
                cx.notify();
            }
        }
    }
    fn color_menu(&self, id: Option<usize>, value: &str, cx: &mut Context<Self>) -> AnyElement {
        let label = PALETTE
            .iter()
            .find(|(key, _, _)| *key == value)
            .map(|(_, label, _)| *label)
            .unwrap_or("默认");
        let owner = cx.entity().downgrade();
        native_button(SharedString::from(format!("color-{id:?}")))
            .small()
            .ghost()
            .label(label)
            .child(icons::icon("select"))
            .dropdown_menu(move |mut m, _, _| {
                if id.is_none() {
                    let owner = owner.clone();
                    m = m.item(PopupMenuItem::new("默认").on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |s, cx| {
                            s.tx = "default".into();
                            cx.notify();
                        });
                    }));
                }
                for (key, label, _) in PALETTE {
                    let owner = owner.clone();
                    m = m.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |s, cx| {
                            if let Some(id) = id {
                                if let Some(row) = s.rows.iter_mut().find(|r| r.id == id) {
                                    row.color = key.into();
                                }
                            } else {
                                s.tx = key.into();
                            }
                            cx.notify();
                        });
                    }));
                }
                m
            })
            .into_any_element()
    }
}
impl Render for ColorEditor {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tx_menu = self.color_menu(None, &self.tx, cx);
        let mut list = div()
            .id("color-rules")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .border_1()
            .border_color(cx.theme().border)
            .relative();
        for (i, row) in self.rows.iter().enumerate() {
            let id = row.id;
            let picker = self.color_menu(Some(id), &row.color, cx);
            list = list.child(
                div()
                    .h(px(32.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.pattern).small().aria_label("匹配关键字")),
                    )
                    .child(
                        Checkbox::new(("color-regex", id))
                            .small()
                            .label("正则")
                            .checked(row.regex)
                            .on_click(cx.listener(move |s, v, _, cx| {
                                if let Some(row) = s.rows.iter_mut().find(|r| r.id == id) {
                                    row.regex = *v;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div().size(px(12.)).rounded_full().bg(rgb(PALETTE
                            .iter()
                            .find(|(key, _, _)| *key == row.color)
                            .map_or(0x888888, |v| v.2))),
                    )
                    .child(picker)
                    .child(
                        native_button(("color-up", id))
                            .small()
                            .ghost()
                            .icon(icons::icon("up"))
                            .tooltip("上移")
                            .disabled(i == 0)
                            .on_click(cx.listener(move |s, _, _, cx| {
                                if let Some(i) = s.rows.iter().position(|r| r.id == id)
                                    && i > 0
                                {
                                    s.rows.swap(i, i - 1);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        native_button(("color-delete", id))
                            .small()
                            .ghost()
                            .icon(icons::icon("trash"))
                            .tooltip("删除")
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.rows.retain(|r| r.id != id);
                                cx.notify();
                            })),
                    ),
            );
        }
        if self.rows.is_empty() {
            list = list.child(
                div()
                    .h_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        icons::icon("palette")
                            .size(px(40.))
                            .text_color(cx.theme().muted_foreground.opacity(0.5)),
                    )
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("没有着色规则"),
                    )
                    .child("接收数据匹配关键字时按指定颜色显示。"),
            );
        }
        div()
            .h(px(420.))
            .text_size(px(13.))
            .line_height(relative(1.2))
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        native_button("color-presets")
                            .small()
                            .label("预设")
                            .child(icons::icon("down"))
                            .dropdown_menu({
                                let owner = cx.entity().downgrade();
                                move |mut m, _, _| {
                                    for (index, label) in
                                        [(0, "ESP-IDF"), (1, "日志等级"), (2, "AT 响应")]
                                    {
                                        let owner = owner.clone();
                                        m = m.item(PopupMenuItem::new(label).on_click(
                                            move |_, w, cx| {
                                                let _ = owner.update(cx, |s, cx| {
                                                    s.rows.clear();
                                                    let rules = match index {
                                                        0 => vec![
                                                            (r"^[EWe] \(", true, "orange"),
                                                            (r"^[IDV] \(", true, "blue"),
                                                        ],
                                                        1 => vec![
                                                            ("ERROR|FATAL", true, "red"),
                                                            ("WARN", false, "orange"),
                                                            ("INFO", false, "blue"),
                                                        ],
                                                        _ => vec![
                                                            ("ERROR", false, "red"),
                                                            ("OK", false, "green"),
                                                        ],
                                                    };
                                                    for (pattern, regex, color) in rules {
                                                        s.add(pattern, regex, color, w, cx);
                                                    }
                                                });
                                            },
                                        ));
                                    }
                                    m
                                }
                            }),
                    )
                    .child(
                        native_button("color-add")
                            .small()
                            .icon(icons::icon("plus"))
                            .label("添加规则")
                            .disabled(self.rows.len() >= 32)
                            .on_click(cx.listener(|s, _, w, cx| s.add("", false, "orange", w, cx))),
                    )
                    .child(div().flex_1())
                    .child("TX 颜色")
                    .child(tx_menu),
            )
            .child(list)
            .when(!self.error.is_empty(), |v| {
                v.child(
                    div()
                        .text_color(cx.theme().danger)
                        .child(self.error.clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        native_button("colors-cancel")
                            .small()
                            .label("取消")
                            .on_click(|_, w, cx| w.close_dialog(cx)),
                    )
                    .child(
                        native_button("colors-save")
                            .small()
                            .primary()
                            .label("保存")
                            .on_click(cx.listener(|s, _, w, cx| s.save(w, cx))),
                    ),
            )
    }
}
