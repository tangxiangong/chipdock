use super::*;
use chipdock_core::serial::program::{self, Rule};

const FIELDS: &[(&str, &str)] = &[
    ("name", "例如 motor-speed"),
    ("template", "AA BB {speed:u16be} {checksum:sum8}"),
    ("display", "例如 电机速度：{speed} rpm"),
    ("checksum_start", "0"),
    ("checksum_end", "自动，直到校验位前"),
    ("response", "无"),
    ("timeout_ms", "1000"),
    ("delimiter", "例如 0D 0A"),
    ("length_field", "长度字段名"),
    ("trailer", "帧尾 HEX"),
    ("sample", "motor-speed 45"),
];
struct ProtocolEditor {
    owner: WeakEntity<Workbench>,
    session: Option<u64>,
    rules: Vec<Rule>,
    selected: Option<usize>,
    fields: BTreeMap<&'static str, Entity<InputState>>,
    result: Entity<TextareaState>,
    error: String,
    syntax: bool,
}
#[derive(Clone)]
struct Token(&'static str);
impl Render for Token {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_2().bg(rgb(0xeeeeee)).rounded_md().child(self.0)
    }
}

impl Workbench {
    pub(super) fn open_protocol(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let rules = match serde_json::from_str::<Vec<Rule>>(
            self.editors["protocol"].read(cx).value().as_ref(),
        ) {
            Ok(r) => r,
            Err(e) => {
                self.report_error(e.to_string(), cx);
                return;
            }
        };
        let owner = cx.entity().downgrade();
        let session = self.active;
        let editor = cx.new(|cx| {
            let fields = FIELDS
                .iter()
                .map(|(key, placeholder)| {
                    (
                        *key,
                        cx.new(|cx| InputState::new(w, cx).placeholder(*placeholder)),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            for field in fields.values() {
                cx.observe(field, |_, _, cx| cx.notify()).detach();
            }
            let mut view = ProtocolEditor {
                owner,
                session,
                rules,
                selected: None,
                fields,
                result: cx.new(|cx| TextareaState::new(w, cx)),
                error: String::new(),
                syntax: false,
            };
            if !view.rules.is_empty() {
                view.select(0, w, cx);
            }
            view
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
            .w(px(860.))
            .p_0()
            .close_button(false)
            .child(editor.clone())
        });
    }
}
impl ProtocolEditor {
    fn value(&self, key: &str, cx: &App) -> String {
        self.fields[key].read(cx).value().to_string()
    }
    fn flush(&mut self, cx: &App) -> Result<()> {
        let Some(i) = self.selected else {
            return Ok(());
        };
        let mut rule = self.rules[i].clone();
        rule.name = self.value("name", cx);
        rule.template = self.value("template", cx);
        rule.display = self.value("display", cx);
        rule.checksum_start = self
            .value("checksum_start", cx)
            .parse()
            .context("校验起点必须为非负整数")?;
        let end = self.value("checksum_end", cx);
        rule.checksum_end = if end.trim().is_empty() {
            None
        } else {
            Some(end.parse().context("校验终点必须为非负整数")?)
        };
        rule.response = self.value("response", cx);
        rule.timeout_ms = self
            .value("timeout_ms", cx)
            .parse()
            .context("等待超时必须为整数")?;
        rule.delimiter = self.value("delimiter", cx);
        rule.length_field = self.value("length_field", cx);
        rule.trailer = self.value("trailer", cx);
        self.rules[i] = rule;
        Ok(())
    }
    fn select(&mut self, index: usize, w: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = self.flush(cx) {
            self.error = e.to_string();
            return;
        }
        self.selected = Some(index);
        let r = &self.rules[index];
        for (key, value) in [
            ("name", r.name.clone()),
            ("template", r.template.clone()),
            ("display", r.display.clone()),
            ("checksum_start", r.checksum_start.to_string()),
            (
                "checksum_end",
                r.checksum_end.map(|v| v.to_string()).unwrap_or_default(),
            ),
            ("response", r.response.clone()),
            ("timeout_ms", r.timeout_ms.to_string()),
            ("delimiter", r.delimiter.clone()),
            ("length_field", r.length_field.clone()),
            ("trailer", r.trailer.clone()),
        ] {
            self.fields[key].update(cx, |s, cx| s.set_value(value, w, cx));
        }
        self.error.clear();
        cx.notify();
    }
    fn add(&mut self, receive: bool, w: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = self.flush(cx) {
            self.error = e.to_string();
            cx.notify();
            return;
        }
        let prefix = if receive { "receive" } else { "command" };
        let mut n = self.rules.len() + 1;
        while self.rules.iter().any(|r| r.name == format!("{prefix}-{n}")) {
            n += 1;
        }
        self.rules.push(Rule {
            name: format!("{prefix}-{n}"),
            template: "AA {value:u16be} {checksum:sum8}".into(),
            receive,
            timeout_ms: 1000,
            ..Default::default()
        });
        self.selected = None;
        self.select(self.rules.len() - 1, w, cx);
    }
    fn insert(&mut self, token: &str, w: &mut Window, cx: &mut Context<Self>) {
        let source = self.value("template", cx);
        let mut words = source
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if token.starts_with("{checksum:") {
            words.retain(|s| !s.starts_with("{checksum:"));
            words.push(token.into());
        } else if token == "AA BB" {
            words.insert(0, token.into());
        } else {
            let position = words
                .iter()
                .position(|s| s.starts_with("{checksum:"))
                .unwrap_or(words.len());
            words.insert(position, token.into());
        }
        self.fields["template"].update(cx, |s, cx| s.set_value(words.join(" "), w, cx));
        cx.notify();
    }
    fn preview(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let result = (|| {
            self.flush(cx)?;
            let receive = self.selected.is_some_and(|i| self.rules[i].receive);
            program::execute(&json!({"rules":self.rules,"operation":if receive{"decode"}else{"encode"},"input":self.value("sample",cx),"incoming":if receive{self.value("sample",cx)}else{String::new()}}).to_string())
        })();
        match result {
            Ok(result) => {
                self.error.clear();
                self.result.update(cx, |s, cx| s.set_value(result, w, cx));
            }
            Err(e) => self.error = e.to_string(),
        };
        cx.notify();
    }
    fn save(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let result = (|| {
            self.flush(cx)?;
            program::execute(&json!({"rules":self.rules,"operation":"validate"}).to_string())?;
            let source = serde_json::to_string_pretty(&self.rules)?;
            self.owner.update(cx, |s, cx| -> Result<()> {
                anyhow::ensure!(s.active == self.session, "会话已切换，请重新打开规则编辑器");
                s.set("protocol", source, w, cx);
                s.serial_action("protocol", cx);
                s.save(w, cx);
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
    fn import(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("导入 JSON 规则".into()),
        });
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.first()
            {
                let result = (|| -> Result<Vec<Rule>> {
                    anyhow::ensure!(
                        std::fs::metadata(path)?.len() <= 1024 * 1024,
                        "规则文件不能超过 1 MiB"
                    );
                    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
                })();
                let _ = view.update_in(cx, |s, w, cx| {
                    match result {
                        Ok(rules) => {
                            s.rules = rules;
                            s.selected = None;
                            if !s.rules.is_empty() {
                                s.select(0, w, cx);
                            }
                        }
                        Err(e) => s.error = e.to_string(),
                    };
                    cx.notify();
                });
            }
        })
        .detach();
    }
    fn field(&self, key: &'static str, label: &'static str) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .min_w_0()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0x888888))
                    .child(label),
            )
            .child(Input::new(&self.fields[key]).small().aria_label(label))
    }
    fn card(&self, title: &'static str, icon: &'static str, cx: &App) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(icons::icon(icon))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(title)),
            )
    }
}
impl Render for ProtocolEditor {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = div().flex_1().min_w_0().min_h_0();
        if let Some(index) = self.selected {
            let receive = self.rules[index].receive;
            let mut form=div().id("protocol-form").size_full().overflow_y_scroll().p_3().flex().flex_col().gap_5()
                .child(div().flex().rounded_md().children([(false,"发送命令"),(true,"接收解析")].into_iter().map(|(mode,label)|native_button(if mode{"receive-rule"}else{"send-rule"}).small().label(label).selected(receive==mode).on_click(cx.listener(move |s,_,_,cx|{if let Some(i)=s.selected{s.rules[i].receive=mode;}cx.notify();})))))
                .child(self.field("name","Alias / 名称"))
                .child(self.card("帧配置","code",cx)
                    .child(div().flex().flex_wrap().gap_2().children([("帧头 AA BB","AA BB"),("u8","{value:u8}"),("u16be","{value:u16be}"),("u16le","{value:u16le}"),("i16be","{value:i16be}"),("u32be","{value:u32be}"),("SUM8","{checksum:sum8}"),("XOR8","{checksum:xor8}"),("CRC16-Modbus","{checksum:crc16modbus}")].into_iter().enumerate().map(|(i,(label,token))|div().id(("drag-token",i)).on_drag(Token(token),|token,_,_,cx|cx.new(|_|token.clone())).child(native_button(("token",i)).small().label(label).bg(control_bg(cx)).border_0().on_click(cx.listener(move |s,_,w,cx|s.insert(token,w,cx)))))))
                    .child(div().text_size(px(11.)).text_color(cx.theme().muted_foreground).child("拖入下方帧模板，或点击插入。帧头放在开头，参数放在校验前；选择校验会替换已有校验。"))
                    .child(div().on_drop(cx.listener(|s,token:&Token,w,cx|s.insert(token.0,w,cx))).child(self.field("template","帧模板")))
                    .child(self.field("display","显示模板 · 可选"))
                    .child(native_button("syntax").small().ghost().icon(icons::icon(if self.syntax{"down"}else{"right"})).label("模板语法与示例").on_click(cx.listener(|s,_,_,cx|{s.syntax = !s.syntax;cx.notify();})))
                    .when(self.syntax,|v|v.child(div().text_size(px(12.)).child("AA BB {speed:u16be} {checksum:sum8}\n字段支持 u8、u16be/le、i16be/le、u32be/le。显示模板通过 {speed} 引用参数。"))));
            let mut options = self
                .card(
                    if receive {
                        "解析选项"
                    } else {
                        "校验与反馈"
                    },
                    "shield",
                    cx,
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .child(self.field("checksum_start", "校验起点")),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(self.field("checksum_end", "校验终点 · 不包含")),
                        ),
                );
            options = options
                .child(self.field("length_field", "长度字段 · 可选"))
                .child(self.field("trailer", "固定帧尾 HEX · 可选"))
                .child(
                    Checkbox::new("length-includes-frame")
                        .label("长度值包含整个帧")
                        .checked(self.rules[index].length_includes_frame)
                        .on_click(cx.listener(|s, v, _, cx| {
                            if let Some(i) = s.selected {
                                s.rules[i].length_includes_frame = *v;
                            }
                            cx.notify();
                        })),
                );
            if receive {
                options = options
                    .child(self.field("delimiter", "文本分隔符 HEX · 可选"))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "文本解析时清空帧模板，显示模板使用 {text}。单帧最多 4096 字节。",
                            ),
                    );
            } else {
                options = options.child(
                    div()
                        .flex()
                        .gap_3()
                        .child(div().flex_1().child(self.field("response", "关联反馈")))
                        .child(
                            div()
                                .flex_1()
                                .child(self.field("timeout_ms", "等待超时 · ms")),
                        ),
                )
            };
            form = form.child(options).child(
                self.card("离线试算", "play", cx)
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap_3()
                            .child(div().flex_1().child(self.field(
                                "sample",
                                if receive {
                                    "输入接收数据 · HEX"
                                } else {
                                    "输入命令 · 包含 alias"
                                },
                            )))
                            .child(
                                native_button("preview")
                                    .small()
                                    .label(if receive { "解析" } else { "试算" })
                                    .disabled(self.value("sample", cx).is_empty())
                                    .on_click(cx.listener(|s, _, w, cx| s.preview(w, cx))),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(cx.theme().muted_foreground)
                            .child("检查生成的字节或解析结果，不会向设备发送数据。"),
                    )
                    .when(!self.result.read(cx).value().is_empty(), |v| {
                        v.child(Textarea::new(&self.result).readonly(true).h(px(100.)))
                    }),
            );
            body = body.child(form);
        } else {
            body = body
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    icons::icon("add-rule")
                        .size(px(32.))
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("添加第一条规则"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child("为常用指令设置名称，或定义接收帧的解析方式。"),
                )
                .child(
                    native_button("add-send-empty")
                        .small()
                        .label("新增发送命令")
                        .on_click(cx.listener(|s, _, w, cx| s.add(false, w, cx))),
                )
                .child(
                    native_button("add-receive-empty")
                        .small()
                        .label("新增接收规则")
                        .on_click(cx.listener(|s, _, w, cx| s.add(true, w, cx))),
                );
        }
        div()
            .w_full()
            .h(px(650.))
            .flex()
            .flex_col()
            .text_size(px(13.))
            .child(
                div()
                    .h(px(78.))
                    .flex_shrink_0()
                    .px_5()
                    .flex()
                    .items_center()
                    .gap_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        icons::icon("terminal")
                            .size(px(24.))
                            .text_color(rgb(0x007aff)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("命令与接收规则"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child("定义发送指令，解析设备反馈"),
                            ),
                    )
                    .child(format!("{} 条规则", self.rules.len())),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .w(px(210.))
                            .min_w_0()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .bg(control_bg(cx))
                            .child(
                                div()
                                    .id("rule-list")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .p_2()
                                    .children(self.rules.iter().enumerate().map(|(i, rule)| {
                                        let name = if self.selected == Some(i) {
                                            self.value("name", cx)
                                        } else {
                                            rule.name.clone()
                                        };
                                        native_button(("rule", i))
                                            .tooltip(name.clone())
                                            .ghost()
                                            .w_full()
                                            .h(px(48.))
                                            .selected(self.selected == Some(i))
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_2()
                                                    .items_start()
                                                    .child(icons::icon(if rule.receive {
                                                        "arrow-down"
                                                    } else {
                                                        "arrow-up"
                                                    }))
                                                    .child(
                                                        div().flex().flex_col().child(name).child(
                                                            div().text_size(px(11.)).child(
                                                                if rule.receive {
                                                                    "接收解析"
                                                                } else {
                                                                    "发送命令"
                                                                },
                                                            ),
                                                        ),
                                                    ),
                                            )
                                            .on_click(
                                                cx.listener(move |s, _, w, cx| s.select(i, w, cx)),
                                            )
                                    })),
                            )
                            .child(
                                div()
                                    .h(px(44.))
                                    .px_3()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .bg(cx.theme().background)
                                    .child(
                                        native_button("add-rule")
                                            .small()
                                            .icon(icons::icon("plus"))
                                            .label("新增")
                                            .dropdown_menu({
                                                let owner = cx.entity().downgrade();
                                                move |mut m, _, _| {
                                                    for (receive, label) in
                                                        [(false, "发送命令"), (true, "接收规则")]
                                                    {
                                                        let owner = owner.clone();
                                                        m = m.item(
                                                            PopupMenuItem::new(label).on_click(
                                                                move |_, w, cx| {
                                                                    let _ = owner.update(
                                                                        cx,
                                                                        |s, cx| {
                                                                            s.add(receive, w, cx)
                                                                        },
                                                                    );
                                                                },
                                                            ),
                                                        );
                                                    }
                                                    m
                                                }
                                            }),
                                    )
                                    .child(div().flex_1())
                                    .child(
                                        native_button("delete-rule")
                                            .small()
                                            .ghost()
                                            .icon(icons::icon("trash"))
                                            .tooltip("删除所选规则")
                                            .disabled(self.selected.is_none())
                                            .on_click(cx.listener(|s, _, w, cx| {
                                                if let Some(i) = s.selected.take() {
                                                    s.rules.remove(i);
                                                    if !s.rules.is_empty() {
                                                        s.select(i.min(s.rules.len() - 1), w, cx);
                                                    }
                                                    cx.notify();
                                                }
                                            })),
                                    ),
                            ),
                    )
                    .child(body),
            )
            .when(!self.error.is_empty(), |v| {
                v.child(
                    div()
                        .px_5()
                        .py_2()
                        .text_color(cx.theme().danger)
                        .child(self.error.clone()),
                )
            })
            .child(
                div()
                    .h(px(54.))
                    .flex_shrink_0()
                    .px_5()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        native_button("import-rules")
                            .small()
                            .icon(icons::icon("import"))
                            .label("导入")
                            .on_click(cx.listener(|s, _, _, cx| s.import(cx))),
                    )
                    .child(
                        native_button("export-rules")
                            .small()
                            .icon(icons::icon("export"))
                            .label("导出")
                            .disabled(self.rules.is_empty())
                            .on_click(cx.listener(|s, _, _, cx| {
                                match s
                                    .flush(cx)
                                    .and_then(|_| Ok(serde_json::to_vec_pretty(&s.rules)?))
                                {
                                    Ok(bytes) => {
                                        let _ = s.owner.update(cx, |o, cx| {
                                            o.export_bytes("ChipDock-rules.json", bytes, cx)
                                        });
                                    }
                                    Err(e) => {
                                        s.error = e.to_string();
                                        cx.notify();
                                    }
                                }
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        native_button("cancel-rules")
                            .small()
                            .label("取消")
                            .on_click(|_, w, cx| w.close_dialog(cx)),
                    )
                    .child(
                        native_button("save-rules")
                            .small()
                            .label("保存规则")
                            .on_click(cx.listener(|s, _, w, cx| s.save(w, cx))),
                    ),
            )
    }
}
