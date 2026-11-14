//! Protocol sheet: the existing 860 × 650 workbench layout, with a cancelable local draft.
use super::*;
use anyhow::{Context as _, Result};
use chipdock_core::serial::program::{self, Rule};
use serde_json::{Value, json};

const FRAGMENTS: &[(&str, &str)] = &[
    ("帧头 AA BB", "AA BB"),
    ("u8", "{value:u8}"),
    ("u16be", "{value:u16be}"),
    ("u16le", "{value:u16le}"),
    ("i16be", "{value:i16be}"),
    ("u32be", "{value:u32be}"),
    ("SUM8", "{checksum:sum8}"),
    ("XOR8", "{checksum:xor8}"),
    ("CRC16-Modbus", "{checksum:crc16modbus}"),
];

#[derive(Clone, PartialEq)]
struct Draft {
    name: String,
    template: String,
    display: String,
    receive: bool,
    checksum_start: String,
    checksum_end: String,
    response: String,
    timeout_ms: String,
    delimiter: String,
    length_field: String,
    length_includes_frame: bool,
    trailer: String,
}
impl From<Rule> for Draft {
    fn from(rule: Rule) -> Self {
        Self {
            name: rule.name,
            template: rule.template,
            display: rule.display,
            receive: rule.receive,
            checksum_start: rule.checksum_start.to_string(),
            checksum_end: rule.checksum_end.map(|n| n.to_string()).unwrap_or_default(),
            response: rule.response,
            timeout_ms: rule.timeout_ms.to_string(),
            delimiter: rule.delimiter,
            length_field: rule.length_field,
            length_includes_frame: rule.length_includes_frame,
            trailer: rule.trailer,
        }
    }
}
impl Draft {
    fn rule(&self) -> Result<Rule> {
        Ok(Rule {
            name: self.name.clone(),
            template: self.template.clone(),
            display: self.display.clone(),
            receive: self.receive,
            checksum_start: self
                .checksum_start
                .trim()
                .parse()
                .context("校验起点必须为非负整数")?,
            checksum_end: if self.checksum_end.trim().is_empty() {
                None
            } else {
                Some(
                    self.checksum_end
                        .trim()
                        .parse()
                        .context("校验终点必须为非负整数")?,
                )
            },
            response: self.response.clone(),
            timeout_ms: self
                .timeout_ms
                .trim()
                .parse()
                .context("等待超时必须为整数")?,
            delimiter: self.delimiter.clone(),
            length_field: self.length_field.clone(),
            length_includes_frame: self.length_includes_frame,
            trailer: self.trailer.clone(),
        })
    }
    fn field(&self, key: &str) -> &str {
        match key {
            "name" => &self.name,
            "template" => &self.template,
            "display" => &self.display,
            "checksum_start" => &self.checksum_start,
            "checksum_end" => &self.checksum_end,
            "response" => &self.response,
            "timeout_ms" => &self.timeout_ms,
            "delimiter" => &self.delimiter,
            "length_field" => &self.length_field,
            "trailer" => &self.trailer,
            _ => "",
        }
    }
    fn set(&mut self, key: &str, value: String) {
        *match key {
            "name" => &mut self.name,
            "template" => &mut self.template,
            "display" => &mut self.display,
            "checksum_start" => &mut self.checksum_start,
            "checksum_end" => &mut self.checksum_end,
            "response" => &mut self.response,
            "timeout_ms" => &mut self.timeout_ms,
            "delimiter" => &mut self.delimiter,
            "length_field" => &mut self.length_field,
            "trailer" => &mut self.trailer,
            _ => return,
        } = value;
    }
}
#[derive(Clone, Copy)]
struct Editor {
    rules: Signal<Vec<Draft>>,
    selected: Signal<Option<usize>>,
    sample: Signal<String>,
    result: Signal<String>,
    layout: Signal<Vec<Value>>,
    failure: Signal<String>,
    syntax: Signal<bool>,
    saving: Signal<bool>,
    testing: Signal<bool>,
    session: Option<u64>,
    template_ref: ElementRef,
}
impl Editor {
    fn field(self, key: &'static str) -> String {
        self.rules.with(|r| {
            self.selected
                .get()
                .and_then(|i| r.get(i))
                .map(|r| r.field(key).to_owned())
                .unwrap_or_default()
        })
    }
    fn set(self, key: &'static str, value: String) {
        if self.saving.get_untracked() {
            return;
        }
        self.rules.update(|r| {
            if let Some(rule) = self.selected.get_untracked().and_then(|i| r.get_mut(i)) {
                rule.set(key, value)
            }
        });
        self.clear_result();
    }
    fn receive(self) -> bool {
        self.rules.with(|r| {
            self.selected
                .get()
                .and_then(|i| r.get(i))
                .is_some_and(|r| r.receive)
        })
    }
    fn clear_result(self) {
        self.result.set(String::new());
        self.layout.set(vec![]);
        self.failure.set(String::new());
    }
    fn select(self, index: usize) {
        self.selected.set(Some(index));
        self.sample.set(String::new());
        self.clear_result();
    }
    fn add(self, receive: bool) {
        if self.saving.get_untracked() {
            return;
        }
        let prefix = if receive { "receive" } else { "command" };
        let name = self.rules.with_untracked(|rules| {
            let mut index = rules.len() + 1;
            while rules.iter().any(|r| r.name == format!("{prefix}-{index}")) {
                index += 1;
            }
            format!("{prefix}-{index}")
        });
        self.rules.update(|rules| {
            rules.push(Draft::from(Rule {
                name,
                receive,
                timeout_ms: 1000,
                template: "AA {value:u16be} {checksum:sum8}".into(),
                ..Default::default()
            }))
        });
        self.select(self.rules.with_untracked(|r| r.len() - 1));
    }
    fn remove(self) {
        self.rules.update(|r| {
            if let Some(i) = self.selected.get_untracked().filter(|i| *i < r.len()) {
                r.remove(i);
            }
        });
        let count = self.rules.with_untracked(Vec::len);
        self.selected.set(if count == 0 {
            None
        } else {
            Some(self.selected.get_untracked().unwrap_or(0).min(count - 1))
        });
        self.clear_result();
    }
    fn snapshot(self) -> Result<Vec<Rule>> {
        self.rules
            .with_untracked(|r| r.iter().map(Draft::rule).collect())
    }
    fn field_view(self, key: &'static str, label: &'static str) -> View {
        let placeholder = match key {
            "name" => "例如 motor-speed",
            "template" => "AA BB {speed:u16be} {checksum:sum8}",
            "display" => "例如 电机速度：{speed} rpm",
            "checksum_end" => "自动，直到校验位前",
            "length_field" => "例如 len",
            "trailer" | "delimiter" => "例如 0D 0A",
            _ => "",
        };
        view().gap(px(6.)).child(caption(label)).child(
            draft_input(
                label,
                move || self.field(key),
                move |value: &str| self.set(key, value.into()),
            )
            .placeholder(placeholder)
            .readonly(move || self.saving.get())
            .when(["name", "template", "delimiter"].contains(&key), |input| {
                input.font_family(mono)
            })
            .when(key == "template", |input| input.multiline(true).h(px(42.))),
        )
    }
    fn preview(self) {
        if self.testing.get_untracked() || self.saving.get_untracked() {
            return;
        }
        let Ok(rules) = self
            .snapshot()
            .map_err(|e| self.failure.set(format!("{e:#}")))
        else {
            return;
        };
        let Some(index) = self.selected.get_untracked() else {
            return;
        };
        let Some(rule) = rules.get(index) else { return };
        let receive = rule.receive;
        let name = rule.name.clone();
        let input = self.sample.get_untracked();
        let snapshot = self.rules.get_untracked();
        self.testing.set(true);
        spawn(async move {
            let sample = input.clone();
            let response=nsh_core::element::blocking(move ||program::execute(&json!({"rules":rules,"operation":"inspect","rule":name,"input":if receive{""}else{&input},"incoming":if receive{&input}else{""}}).to_string())).await;
            self.testing.try_set(false);
            if self.selected.get_untracked() != Some(index)
                || self.sample.get_untracked() != sample
                || self.rules.get_untracked() != snapshot
            {
                return;
            }
            match response {
                Ok(Ok(text)) => match serde_json::from_str::<Value>(&text) {
                    Ok(value) => {
                        self.layout
                            .set(value["layout"].as_array().cloned().unwrap_or_default());
                        self.result.set(format_preview(&value));
                        self.failure.set(String::new());
                    }
                    Err(e) => self.failure.set(e.to_string()),
                },
                Ok(Err(e)) => self.failure.set(format!("{e:#}")),
                Err(e) => self.failure.set(e.to_string()),
            }
        });
    }
    fn save(self, store: Store) {
        if self.saving.get_untracked() {
            return;
        }
        if store.read(|s| s.active) != self.session {
            self.failure.set("会话已切换，请重新打开规则编辑器".into());
            return;
        }
        let rules = match self.snapshot() {
            Ok(r) => r,
            Err(e) => {
                self.failure.set(format!("{e:#}"));
                return;
            }
        };
        let Some(id) = self.session else {
            self.failure.set("没有活动会话".into());
            return;
        };
        self.saving.set(true);
        spawn(async move {
            let result = nsh_core::element::blocking(move || -> Result<String> {
                program::execute(&json!({"rules":rules,"operation":"validate"}).to_string())?;
                let source = serde_json::to_string_pretty(&rules)?;
                chipdock_service::serial::serial_workbench(
                    id,
                    &json!({"action":"protocol","rules":rules}).to_string(),
                )?;
                Ok(source)
            })
            .await;
            self.saving.try_set(false);
            match result {
                Ok(Ok(source)) => {
                    if store.read(|s| s.active) != Some(id) {
                        self.failure.set("会话已切换，请重新打开规则编辑器".into());
                        return;
                    }
                    let saved = store.change(|s| {
                        s.set("protocol", source);
                        s.save()
                    });
                    match saved {
                        Ok(()) => tools::close(store),
                        Err(e) => self.failure.set(format!("{e:#}")),
                    }
                }
                Ok(Err(e)) => self.failure.set(format!("{e:#}")),
                Err(e) => self.failure.set(e.to_string()),
            }
        });
    }
    fn import(self, store: Store) {
        if store.file_dialog.get_untracked() {
            return;
        }
        store.file_dialog.set(true);
        store.native_sheet.set(cfg!(target_os = "macos"));
        store.spawn(async move {
            let imported = tools::read_json(super::file_dialogs::OpenKind::DocumentJson).await;
            store.native_sheet.set(false);
            store.file_dialog.set(false);
            if self.rules.try_get().is_none() {
                return;
            }
            match imported {
                Ok(Some(value)) => match serde_json::from_value::<Vec<Rule>>(value) {
                    Ok(rules) => {
                        if rules.len() > 128 {
                            self.failure.set("协议规则超过 128 条".into());
                            return;
                        }
                        store.file_dialog.set(true);
                        store.native_sheet.set(cfg!(target_os = "macos"));
                        let answer = nsh_base::alert_dialog()
                            .title("用导入文件替换当前编辑的全部规则？保存后才会生效。")
                            .cancel("cancel", "取消")
                            .destructive("replace", "替换规则")
                            .show()
                            .await;
                        store.native_sheet.set(false);
                        store.file_dialog.set(false);
                        if self.rules.try_get().is_none() {
                            return;
                        }
                        if matches!(answer,Ok(r) if r.action.as_deref()==Some("replace")) {
                            self.rules.set(rules.into_iter().map(Draft::from).collect());
                            let count = self.rules.with_untracked(Vec::len);
                            self.selected.set((count > 0).then_some(0));
                            self.clear_result();
                        }
                    }
                    Err(e) => self.failure.set(e.to_string()),
                },
                Ok(None) => {}
                Err(e) => self.failure.set(e),
            }
        });
    }
}

pub(super) fn page(store: Store) -> View {
    let initial = serde_json::from_str::<Vec<Rule>>(&store.field("protocol"));
    let (rules, error) = match initial {
        Ok(r) => (
            r.into_iter().map(Draft::from).collect::<Vec<_>>(),
            String::new(),
        ),
        Err(e) => (vec![], e.to_string()),
    };
    let selected = (!rules.is_empty()).then_some(0);
    let editor = Editor {
        rules: signal(rules),
        selected: signal(selected),
        sample: signal(String::new()),
        result: signal(String::new()),
        layout: signal(vec![]),
        failure: signal(error),
        syntax: signal(false),
        saving: signal(false),
        testing: signal(false),
        session: store.tool_session.get_untracked(),
        template_ref: element_ref(),
    };
    view()
        .w(px(860.))
        .h(px(650.))
        .bg(move || {
            theme()
                .color("protocol-background")
                .unwrap_or(theme().background)
        })
        .text_size(body_size)
        .line_height(nsh_core::LineHeight::Multiple(1.2))
        .child(
            view()
                .p_5()
                .flex_row()
                .items_center()
                .gap_3()
                .border_b_1()
                .border_color(hairline)
                .child(
                    icons::icon("terminal")
                        .size(px(24.))
                        .scale_y(0.86)
                        .translate_y(px(0.25))
                        .text_color(move || theme().primary),
                )
                .child(
                    view()
                        .flex_1()
                        .gap_1()
                        .child(
                            text("命令与接收规则")
                                .text_size(px(15.))
                                .font_weight(FontWeight::SEMIBOLD),
                        )
                        .child(caption("定义发送指令，解析设备反馈")),
                )
                .child(
                    text(move || format!("{} 条规则", editor.rules.with(Vec::len)))
                        .text_size(small_size)
                        .text_color(move || theme().muted_foreground),
                ),
        )
        .child(
            view()
                .flex_1()
                .min_h_0()
                .flex_row()
                .child(sidebar(editor, store))
                .child(
                    when(
                        move || editor.selected.get().is_some(),
                        move || form(editor, store),
                    )
                    .otherwise(move || empty(editor)),
                ),
        )
        .child(when(
            move || !editor.failure.get().is_empty(),
            move || {
                view().px_5().py_2().max_h(px(76.)).child(
                    text(move || editor.failure.get())
                        .text_size(caption_size)
                        .text_color(move || theme().destructive),
                )
            },
        ))
        .child(
            view()
                .h(px(54.))
                .flex_shrink_0()
                .px_5()
                .flex_row()
                .items_center()
                .gap_3()
                .border_t_1()
                .border_color(hairline)
                .child(footer_button(
                    "import",
                    "导入",
                    move || editor.saving.get(),
                    move || editor.import(store),
                ))
                .child(footer_button(
                    "export",
                    "导出",
                    move || editor.saving.get() || editor.rules.with(Vec::is_empty),
                    move || match editor
                        .snapshot()
                        .and_then(|r| Ok(serde_json::to_vec_pretty(&r)?))
                    {
                        Ok(bytes) => store.export_as(
                            "ChipDock-Protocol.json",
                            bytes,
                            super::file_dialogs::ExportKind::DocumentData,
                        ),
                        Err(e) => editor.failure.set(format!("{e:#}")),
                    },
                ))
                .child(view().flex_1())
                .child(action(
                    "取消",
                    Variant::Default,
                    move || editor.saving.get(),
                    move || tools::close(store),
                ))
                .child(action(
                    move || {
                        if editor.saving.get() {
                            "保存中…"
                        } else {
                            "保存规则"
                        }
                    },
                    Variant::Primary,
                    move || editor.saving.get(),
                    move || editor.save(store),
                )),
        )
}
fn footer_button(
    icon: &'static str,
    label: &'static str,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    nsh_base::button()
        .accessibility_label(label)
        .disabled(disabled)
        .on_press(press)
        .w(px(72.5))
        .h(px(24.))
        .p_0()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(5.))
        .rounded(px(7.))
        .bg(move || {
            theme()
                .color("protocol-button")
                .unwrap_or(theme().secondary)
        })
        .disabled_style(|s| s.opacity(0.5))
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .child(icons::icon(icon).size(px(17.5)))
        .child(text(label).text_size(body_size))
}

fn sidebar(editor: Editor, store: Store) -> View {
    let window_active = control_active();
    let menu_tracking = native_menu_tracking();
    let active =
        computed(move || window_active.get() && !store.file_dialog.get() && !menu_tracking.get());
    view()
        .w(px(211.))
        .min_w_0()
        .border_r_1()
        .border_color(hairline)
        .bg(move || {
            theme()
                .color(if menu_tracking.get() {
                    "protocol-sidebar-inactive"
                } else {
                    "protocol-sidebar"
                })
                .unwrap_or(theme().sidebar)
        })
        .child(
            virtual_list(
                move || editor.rules.with(Vec::len),
                move |at| {
                    let rule = computed(move || editor.rules.with(|r| r.get(at.index()).cloned()));
                    nsh_base::button()
                        .selected(move || editor.selected.get() == Some(at.index()))
                        .on_press(move || editor.select(at.index()))
                        .disabled(move || editor.saving.get())
                        .h(px(48.))
                        .w_full()
                        .px_3()
                        .flex_row()
                        .gap_2()
                        .items_center()
                        .rounded(move || theme().radius)
                        .hover(|s| s.bg(move || theme().accent))
                        .active(|s| s.bg(move || theme().muted))
                        .selected_style(|s| s.bg(move || theme().foreground.opacity(0.12)))
                        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
                        .child(
                            view()
                                .size(px(17.))
                                .ml(px(-1.5))
                                .mr(px(0.5))
                                .mb(px(14.5))
                                .rounded_full()
                                .border(px(1.5))
                                .border_color(move || {
                                    if active.get() {
                                        theme().primary
                                    } else {
                                        theme()
                                            .color("protocol-tint-inactive")
                                            .unwrap_or(theme().muted_foreground)
                                    }
                                })
                                .text_color(move || {
                                    if active.get() {
                                        theme().primary
                                    } else {
                                        theme()
                                            .color("protocol-tint-inactive")
                                            .unwrap_or(theme().muted_foreground)
                                    }
                                })
                                .items_center()
                                .justify_center()
                                .child(
                                    when(
                                        move || {
                                            rule.with(|rule| {
                                                rule.as_ref().is_some_and(|rule| rule.receive)
                                            })
                                        },
                                        || icons::icon("arrow-down").size(px(12.)),
                                    )
                                    .otherwise(|| icons::icon("arrow-up").size(px(12.))),
                                ),
                        )
                        .child(
                            view()
                                .flex_1()
                                .gap(px(3.))
                                .child(
                                    text(move || {
                                        rule.with(|r| {
                                            r.as_ref().map(|r| r.name.clone()).unwrap_or_default()
                                        })
                                    })
                                    .text_size(body_size)
                                    .font_weight(FontWeight::MEDIUM)
                                    .number_of_lines(1)
                                    .text_overflow(nsh_core::EllipsizeMode::Middle),
                                )
                                .child(
                                    text(move || {
                                        rule.with(|r| {
                                            if r.as_ref().is_some_and(|r| r.receive) {
                                                "接收解析"
                                            } else {
                                                "发送命令"
                                            }
                                        })
                                    })
                                    .text_size(px(10.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(move || theme().muted_foreground),
                                ),
                        )
                },
            )
            .estimated_item_size(px(48.))
            .content_padding(px(10.))
            .flex_1(),
        )
        .child(
            view()
                .h(px(44.))
                .px_3()
                .flex_row()
                .items_center()
                .gap_2()
                .border_t_1()
                .border_color(hairline)
                .bg(move || {
                    theme()
                        .color("protocol-background")
                        .unwrap_or(theme().background)
                })
                .child(native_menu_small_disabled(
                    view()
                        .w(px(75.5))
                        .h(px(20.))
                        .pl(px(8.))
                        .pr(px(7.))
                        .flex_row()
                        .items_center()
                        .gap(px(2.))
                        .label("新增规则")
                        .rounded(move || theme().radius)
                        .bg(move || {
                            theme()
                                .color("protocol-button")
                                .unwrap_or(theme().secondary)
                        })
                        .child(icons::icon("plus").size(px(13.5)).ml(px(0.5)))
                        .child(text("新增").text_size(small_control_size))
                        .child(view().flex_1())
                        .child(icons::icon("down").size(px(10.))),
                    move || editor.saving.get(),
                    move || {
                        vec![
                            nsh_core::element::desktop::MenuItem::new("发送命令")
                                .on_select(move || editor.add(false)),
                            nsh_core::element::desktop::MenuItem::new("接收规则")
                                .on_select(move || editor.add(true)),
                        ]
                    },
                ))
                .child(view().flex_1())
                .child(
                    nsh_base::button()
                        .accessibility_label("删除所选规则")
                        .on_press(move || editor.remove())
                        .disabled(move || editor.saving.get() || editor.selected.get().is_none())
                        .w(px(32.))
                        .h(px(20.))
                        .p_0()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.))
                        .bg(move || theme().color("protocol-token").unwrap_or(theme().secondary))
                        .disabled_style(|s| s.opacity(0.35))
                        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
                        .child(icons::icon("trash").size(px(13.))),
                ),
        )
}
fn empty(editor: Editor) -> View {
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_2()
        .text_color(move || theme().muted_foreground)
        .child(
            icons::icon("add-rule")
                .size(px(44.))
                .mb(px(12.))
                .text_color(move || {
                    theme()
                        .color("empty-symbol")
                        .unwrap_or(theme().muted_foreground)
                }),
        )
        .child(
            text("添加第一条规则")
                .text_size(px(15.))
                .font_weight(FontWeight::BOLD),
        )
        .child(text("为常用指令设置名称，或定义接收帧的解析方式。").text_size(px(15.)))
        .child(
            view()
                .mt_2()
                .flex_row()
                .gap_2()
                .child(action(
                    "新增发送命令",
                    Variant::Primary,
                    || false,
                    move || editor.add(false),
                ))
                .child(action(
                    "新增接收规则",
                    Variant::Default,
                    || false,
                    move || editor.add(true),
                )),
        )
}
fn card(title: impl IntoReactive<SharedString>, symbol: &str, content: View) -> View {
    view()
        .gap(px(3.))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .px_2()
                .child(icons::icon(symbol))
                .child(
                    text(title)
                        .text_size(body_size)
                        .font_weight(FontWeight::SEMIBOLD),
                ),
        )
        .child(
            content
                .p(px(13.))
                .bg(move || theme().color("protocol-group").unwrap_or(theme().card))
                .rounded(move || px(theme().number("radius-lg").unwrap_or(10.))),
        )
}
fn form(editor: Editor, store: Store) -> impl IntoElement {
    let window_active = control_active();
    let menu_tracking = native_menu_tracking();
    let active =
        computed(move || window_active.get() && !store.file_dialog.get() && !menu_tracking.get());
    let identity = view()
        .gap_3()
        .child(
            nsh_base::tabs()
                .w(px(220.))
                .flex_row()
                .justify_center()
                .children([(false, "发送命令"), (true, "接收解析")].into_iter().map(
                    move |(receive, label)| {
                        nsh_base::tab()
                            .disabled(move || editor.saving.get())
                            .selected(move || editor.receive() == receive)
                            .on_press(move || {
                                editor.rules.update(|rules| {
                                    if let Some(rule) = editor
                                        .selected
                                        .get_untracked()
                                        .and_then(|i| rules.get_mut(i))
                                    {
                                        rule.receive = receive;
                                        if receive {
                                            rule.response.clear()
                                        } else {
                                            rule.delimiter.clear()
                                        }
                                    }
                                });
                                editor.clear_result();
                            })
                            .h(px(24.))
                            .px_3()
                            .bg(move || {
                                theme()
                                    .color("protocol-button")
                                    .unwrap_or(theme().secondary)
                            })
                            .rounded(move || theme().radius)
                            .selected_style(|style| {
                                style
                                    .bg(move || emphasized_fill(true, false, active.get(), false))
                                    .text_color(move || {
                                        emphasized_ink(true, false, active.get(), false)
                                    })
                            })
                            .hover(|style| style.bg(move || theme().accent))
                            .child(text(label).text_size(body_size))
                    },
                )),
        )
        .child(editor.field_view("name", "Alias / 名称"));
    let frame=view().gap_3()
        .child(view().grid().grid_template_columns("repeat(auto-fit, minmax(110px, 1fr))").gap_2()
            .children(FRAGMENTS.iter().map(move |(label,token)|fragment_token(editor,label,token))))
        .child(text("拖入下方帧模板，或点击插入。帧头放在开头，参数放在校验前；选择校验会替换已有校验。")
            .text_size(px(10.)).line_height(px(13.)).text_color(move ||theme().muted_foreground))
        .child(editor.field_view("template","帧模板").p(px(6.)).element_ref(&editor.template_ref))
        .child(editor.field_view("display","显示模板 · 可选"))
        // Disclosure content uses its own native 6pt gap, separate from the form's 12pt spacing.
        .child(view().gap(px(6.)).child(view().role(AccessibilityRole::Button).focusable(true).disabled(move || editor.saving.get())
            .on_press(move |_: &()| { if !editor.saving.get_untracked() { editor.syntax.update(|value|*value = !*value) } })
            .h(px(22.)).flex_row().items_center().gap_1().label("模板语法与示例")
            .accessibility_state(move || AccessibilityState { expanded: Some(editor.syntax.get()), ..Default::default() })
            .child(text(move ||if editor.syntax.get(){"⌄"}else{"›"}).text_size(body_size))
            .child(text("模板语法与示例").text_size(caption_size)))
        .child(when(editor.syntax,||view().items_center().child(
            view().gap(px(6.)).items_start().selectable(true)
                .w(move || measure_text(&format!("{}px \"{}\"",10. * font_scale(), theme().font_sans.clone().unwrap_or_else(|| ".AppleSystemUIFont".into())), "整数：u8 / i8，u16 / i16 / u32 / i32 配合 be（大端）或 le（小端）。参数顺序与模板一致。"))
                .text_size(px(10.)).line_height(px(13.))
                .text_color(move ||theme().muted_foreground)
                .child(text("AA BB {speed:u16be:0:3000} {checksum:sum8}").font_family(mono).text_size(px(10.)))
                .child(text("整数：u8 / i8，u16 / i16 / u32 / i32 配合 be（大端）或 le（小端）。参数顺序与模板一致。"))
                .child(text("校验：sum8、xor8、crc16modbus，放在帧尾。"))
        ))));
    let options = view()
        .gap_3()
        .child(
            view()
                .flex_row()
                .items_start()
                .gap_4()
                .child(editor.field_view("checksum_start", "校验起点").flex_1())
                .child(
                    editor
                        .field_view("checksum_end", "校验终点 · 不包含")
                        .flex_1(),
                ),
        )
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    editor
                        .field_view("length_field", "长度字段 · 可选")
                        .flex_1(),
                )
                .child(editor.field_view("trailer", "固定帧尾 HEX · 可选").flex_1()),
        )
        .child(length_toggle(editor))
        .child(
            when(
                move || editor.receive(),
                move || {
                    view()
                        .gap_3()
                        .child(editor.field_view("delimiter", "文本分隔符 HEX · 可选"))
                        .child(
                            text("文本解析时清空帧模板，显示模板使用 {text}。单帧最多 4096 字节。")
                                .text_size(px(10.))
                                .line_height(px(13.))
                                .text_color(move || theme().muted_foreground),
                        )
                },
            )
            .otherwise(move || {
                view()
                    .flex_row()
                    .items_start()
                    .gap_4()
                    .child(response_picker(editor).flex_1())
                    .child(editor.field_view("timeout_ms", "等待超时 · ms").flex_1())
            }),
        );
    let test = view()
        .gap_3()
        .child(
            view()
                .flex_row()
                .items_end()
                .gap_3()
                .child(
                    view()
                        .flex_1()
                        .gap(px(6.))
                        .child(caption(move || {
                            if editor.receive() {
                                "输入接收数据 · HEX"
                            } else {
                                "输入命令 · 包含 alias"
                            }
                        }))
                        .child(
                            draft_input(
                                "离线试算输入",
                                move || editor.sample.get(),
                                move |value: &str| editor.sample.set(value.into()),
                            )
                            .placeholder(move || {
                                if editor.receive() {
                                    "CC DD 00 2D D6"
                                } else {
                                    "motor-speed 45"
                                }
                            })
                            .font_family(mono)
                            .readonly(move || editor.saving.get()),
                        ),
                )
                .child(when(
                    move || {
                        editor.receive()
                            && store.read(|state| {
                                editor
                                    .session
                                    .and_then(|id| state.sessions.get(&id))
                                    .is_some_and(|session| {
                                        session.frames.iter().any(|frame| {
                                            matches!(
                                                frame.direction,
                                                chipdock_core::serial::Direction::Receive
                                            ) && !frame.bytes.is_empty()
                                        })
                                    })
                            })
                    },
                    move || {
                        action(
                            "用最新 RX",
                            Variant::Default,
                            move || {
                                store.read(|state| {
                                    editor
                                        .session
                                        .and_then(|id| state.sessions.get(&id))
                                        .is_none_or(|session| session.frames.is_empty())
                                })
                            },
                            move || {
                                let latest = store.read(|state| {
                                    editor
                                        .session
                                        .and_then(|id| state.sessions.get(&id))
                                        .and_then(|session| {
                                            session.frames.iter().rev().find(|frame| {
                                                matches!(
                                                    frame.direction,
                                                    chipdock_core::serial::Direction::Receive
                                                )
                                            })
                                        })
                                        .map(|frame| frame.hex.clone())
                                        .unwrap_or_default()
                                });
                                editor.sample.set(latest);
                            },
                        )
                    },
                ))
                .child(form_action(
                    move || {
                        if editor.testing.get() {
                            "处理中…"
                        } else if editor.receive() {
                            "解析"
                        } else {
                            "试算"
                        }
                    },
                    move || editor.testing.get() || editor.sample.get().trim().is_empty(),
                    move || editor.preview(),
                )),
        )
        .child(when(
            move || !editor.layout.with(Vec::is_empty),
            move || {
                scroll()
                    .horizontal()
                    .child(view().flex_row().gap_2().child(each(
                        editor.layout,
                        |span: &Value| span["start"].as_u64().unwrap_or(0),
                        move |span, index| {
                            view()
                                .selectable(true)
                                .p(px(6.))
                                .gap_1()
                                .rounded(px(4.))
                                .bg(move || {
                                    theme()
                                        .color(match index.get() % 4 {
                                            0 => "protocol-span-blue",
                                            1 => "protocol-span-teal",
                                            2 => "protocol-span-orange",
                                            _ => "protocol-span-purple",
                                        })
                                        .unwrap_or(theme().accent)
                                })
                                .child(
                                    text(move || {
                                        span.with(|value| {
                                            format!(
                                                "{} [{}..<{}]",
                                                value["name"].as_str().unwrap_or(""),
                                                value["start"].as_u64().unwrap_or(0),
                                                value["start"].as_u64().unwrap_or(0)
                                                    + value["bytes"].as_u64().unwrap_or(0)
                                            )
                                        })
                                    })
                                    .text_size(px(10.)),
                                )
                                .child(
                                    text(move || {
                                        span.with(|value| {
                                            value["hex"].as_str().unwrap_or("").to_owned()
                                        })
                                    })
                                    .font_family(mono)
                                    .text_size(px(10.)),
                                )
                        },
                    )))
            },
        ))
        .child(
            when(
                move || editor.result.get().is_empty(),
                || {
                    text("检查生成的字节或解析结果，不会向设备发送数据。")
                        .text_size(caption_size)
                        .text_color(move || theme().muted_foreground)
                },
            )
            .otherwise(move || {
                view()
                    .selectable(true)
                    .p_3()
                    .rounded(move || theme().radius)
                    .bg(move || {
                        theme()
                            .color("protocol-background")
                            .unwrap_or(theme().background)
                    })
                    .child(
                        text(move || editor.result.get())
                            .font_family(mono)
                            .text_size(caption_size),
                    )
            }),
        );
    scroll().flex_1().min_w_0().child(
        view()
            .p_5()
            .gap_5()
            .child(identity)
            .child(card("帧配置", "code", frame))
            .child(card(
                move || {
                    if editor.receive() {
                        "解析选项"
                    } else {
                        "校验与反馈"
                    }
                },
                "shield",
                options,
            ))
            .child(card("离线试算", "play", test)),
    )
}
fn length_toggle(editor: Editor) -> impl IntoElement {
    let checked = move || {
        editor.rules.with(|rules| {
            editor
                .selected
                .get()
                .and_then(|index| rules.get(index))
                .is_some_and(|rule| rule.length_includes_frame)
        })
    };
    nsh_base::checkbox()
        .checked(checked)
        .disabled(move || editor.saving.get() || editor.field("length_field").is_empty())
        .on_change(move |value: &bool| {
            editor.rules.update(|rules| {
                if let Some(rule) = editor
                    .selected
                    .get_untracked()
                    .and_then(|index| rules.get_mut(index))
                {
                    rule.length_includes_frame = *value;
                }
            });
            editor.clear_result();
        })
        .accessibility_label("长度表示整帧字节数")
        .flex_row()
        .items_center()
        .gap(px(6.))
        .disabled_style(|style| style.opacity(0.45))
        .indicator_style(|style| {
            style
                .size(px(16.))
                .rounded(px(4.))
                .bg(move || theme().color("control").unwrap_or(theme().secondary))
                .checked(|style| style.bg(move || theme().primary))
        })
        .indicator(
            text(move || if checked() { "✓" } else { "" })
                .text_size(caption_size)
                .text_color(move || theme().primary_foreground),
        )
        .child(text("长度表示整帧字节数").text_size(body_size))
}
fn response_picker(editor: Editor) -> View {
    view()
        .gap(px(6.))
        .items_start()
        .child(caption("关联反馈"))
        .child(
            nsh_base::select()
                .items(move || {
                    editor.rules.with(|r| {
                        std::iter::once(nsh_base::SelectItem::new("", "无"))
                            .chain(
                                r.iter().filter(|r| r.receive).map(|r| {
                                    nsh_base::SelectItem::new(r.name.clone(), r.name.clone())
                                }),
                            )
                            .collect::<Vec<_>>()
                    })
                })
                .value(move || Some(SharedString::from(editor.field("response"))))
                .on_change(move |value: &Option<SharedString>| {
                    if let Some(value) = value {
                        editor.set("response", value.to_string())
                    }
                })
                .disabled(move || editor.saving.get())
                .accessibility_label("关联反馈")
                .popup_style(popup)
                .item_class("focus:bg-accent selected:font-medium")
                .render_item(|item: &nsh_base::SelectEntry| {
                    view()
                        .px_2()
                        .py_1()
                        .child(text(item.title.clone()).text_size(body_size))
                })
                .trigger(
                    view()
                        .h(px(24.))
                        .px(px(12.))
                        .gap(px(12.))
                        .bg(move || theme().color("control").unwrap_or(theme().secondary))
                        .rounded(move || theme().radius)
                        .flex_row()
                        .items_center()
                        .child(
                            text(move || {
                                let value = editor.field("response");
                                if value.is_empty() {
                                    "无".into()
                                } else {
                                    value
                                }
                            })
                            .text_size(body_size),
                        )
                        .child(icons::icon("select").size(px(12.))),
                ),
        )
}
fn fragment_token(editor: Editor, label: &'static str, token: &'static str) -> View {
    let origin = stored(None::<Point<Points>>);
    view()
        .items_start()
        .tooltip(text(token).font_family(mono).text_size(caption_size))
        .pan_enabled(true)
        .on_press_down(move |event: &nsh_core::PressEvent| {
            origin.replace(Some(event.page_position));
        })
        .on_pan_end(move |event: &PanEvent| {
            if event.cancelled {
                return;
            }
            let Some(start) = origin.get() else { return };
            let point = Point {
                x: start.x + event.translation.x,
                y: start.y + event.translation.y,
            };
            spawn(async move {
                if let Some(measure) = editor.template_ref.measure().await {
                    let bounds = Bounds {
                        origin: measure.page,
                        size: measure.frame.size,
                    };
                    if bounds.contains(&point) {
                        editor.set(
                            "template",
                            insert_fragment(&editor.field("template"), token),
                        );
                    }
                }
            });
        })
        .child(
            nsh_base::button()
                .disabled(move || editor.saving.get())
                .on_press(move || {
                    editor.set(
                        "template",
                        insert_fragment(&editor.field("template"), token),
                    )
                })
                .accessibility_label(label)
                .h(px(24.))
                .px_3()
                .min_w(move || {
                    measure_text(
                        &format!(
                            "{}px \"{}\"",
                            body_size().0 * font_scale(),
                            theme()
                                .font_sans
                                .clone()
                                .unwrap_or_else(|| ".AppleSystemUIFont".into())
                        ),
                        label,
                    ) + px(24.)
                })
                .rounded(move || theme().radius)
                .bg(move || theme().color("protocol-token").unwrap_or(theme().secondary))
                .hover(|style| style.bg(move || theme().accent))
                .active(|style| style.bg(move || theme().muted))
                .focus_visible(|style| style.outline(px(2.)).outline_color(move || theme().ring))
                .child(text(label).text_size(body_size).number_of_lines(1)),
        )
}
fn insert_fragment(template: &str, token: &str) -> String {
    let mut words = template
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if token.starts_with("{checksum:") {
        words.retain(|word| !word.starts_with("{checksum:"));
        words.push(token.into());
    } else if token == "AA BB" {
        words.splice(0..0, ["AA".into(), "BB".into()]);
    } else {
        let fragment = if token.starts_with("{value:") {
            let mut name = "value".to_owned();
            let mut suffix = 2;
            while words
                .iter()
                .any(|word| word.starts_with(&format!("{{{name}:")))
            {
                name = format!("value{suffix}");
                suffix += 1;
            }
            token.replacen("{value:", &format!("{{{name}:"), 1)
        } else {
            token.into()
        };
        let index = words
            .iter()
            .position(|word| word.starts_with("{checksum:"))
            .unwrap_or(words.len());
        words.insert(index, fragment);
    }
    words.join(" ")
}
fn format_preview(value: &Value) -> String {
    let mut lines = vec![];
    if let Some(hex) = value["hex"].as_str().filter(|s| !s.is_empty()) {
        lines.push(hex.to_owned());
    }
    if let Some(records) = value["records"].as_array() {
        for record in records {
            lines.push(format!(
                "{}: {} [{}]",
                record["name"].as_str().unwrap_or(""),
                record["message"].as_str().unwrap_or(""),
                record["hex"].as_str().unwrap_or("")
            ));
        }
    }
    if let Some(buffer) = value["buffer"].as_str().filter(|s| !s.is_empty()) {
        lines.push(format!("等待剩余字节：{buffer}"));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragments_keep_header_parameters_and_one_checksum_in_order() {
        let source = insert_fragment("{value:u8} {checksum:sum8}", "{value:u16be}");
        let source = insert_fragment(&source, "AA BB");
        assert_eq!(
            insert_fragment(&source, "{checksum:xor8}"),
            "AA BB {value:u8} {value2:u16be} {checksum:xor8}"
        );
    }
    #[test]
    fn invalid_numeric_drafts_are_preserved_and_rejected_on_save() {
        let mut draft = Draft::from(Rule {
            timeout_ms: 1000,
            ..Default::default()
        });
        draft.checksum_start = "-1".into();
        assert!(draft.rule().is_err());
        assert_eq!(draft.checksum_start, "-1");
    }
}
