use super::*;
use serde_json::Value;

#[path = "tool_feedback.rs"]
mod feedback;
use feedback::Feedback;

pub(super) fn open(store: Store, key: &'static str) {
    store.change(|s| s.output.clear());
    store.tool_session.set(store.read(|s| s.active));
    store.tool.set(Some(key));
    store.modal.set(true);
}
pub(super) fn close(store: Store) {
    store.tool.set(None);
    store.modal.set(false);
}

/// Per-dialog packet and framing drafts and the captured session's feedback.
#[derive(Clone, Copy)]
struct ToolSession {
    store: Store,
    packets: Signal<String>,
    framing_mode: Signal<String>,
    framing_value: Signal<String>,
    feedback: Feedback,
}

impl ToolSession {
    fn current() -> Self {
        use_context().expect("serial tool controls require SerialToolSession")
    }
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct SerialToolSessionProps {
    /// Visible tool, used to invalidate stale async feedback on tab changes.
    pub page: Prop<SharedString>,
    /// The tool page and its controls.
    pub children: Prop<Children>,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub enum SerialToolSessionEvent {
    /// Feedback and job geometry for the current tool.
    #[args(page, result, error, label_height, job_height)]
    FeedbackChange(SharedString, bool, bool, f32, f32),
}

/// Owns the session-specific drafts and operation revisions while TSX arranges the tools.
#[component]
pub fn SerialToolSession(
    props: SerialToolSessionProps,
    cx: Scope<SerialToolSessionEvent>,
) -> impl IntoElement {
    let (store, _) = workbench_context();
    let feedback = Feedback::new(store);
    provide_context(components::ModalControls);
    provide_context(ToolSession {
        store,
        packets: signal(store.field("packets")),
        framing_mode: signal(store.field("frame_mode")),
        framing_value: signal(store.field("frame_value")),
        feedback,
    });
    effect(move || {
        props.page.get();
        untracked(move || feedback.reset(store));
    });
    let metrics = computed(move || {
        let page = props.page.get();
        let job_height = if matches!(page.as_ref(), "packets" | "file" | "macro" | "replies") {
            feedback::job_height(store, feedback)
        } else {
            0.
        };
        (
            page,
            !feedback.result.get().is_empty(),
            !feedback.error.get().is_empty(),
            feedback.label_height.get(),
            job_height,
        )
    });
    effect(move || {
        let (page, result, error, label_height, job_height) = metrics.get();
        cx.emit(SerialToolSessionEvent::FeedbackChange(
            page,
            result,
            error,
            label_height,
            job_height,
        ));
    });
    view().child(props.children)
}

pub(super) fn register(registry: &mut Registry) {
    registry
        .register::<ProfileSession>()
        .register::<ProfileName>()
        .register::<ProfileAction>()
        .register::<SerialToolSession>()
        .register::<ToolParameter>()
        .register::<ToolCommandButton>()
        .register::<ToolOutcome>()
        .register::<SerialJobStatus>()
        .register::<ToolDone>();
    super::protocol::register(registry);
    super::colors::register(registry);
}

fn tool_action(
    label: impl IntoReactive<SharedString>,
    feedback: Feedback,
    disabled: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    form_action(
        label,
        move || feedback.running.get() || disabled(),
        on_press,
    )
}
fn run(
    store: Store,
    key: &'static str,
    label: impl IntoReactive<SharedString>,
    feedback: Feedback,
) -> impl IntoElement {
    tool_action(
        label,
        feedback,
        move || {
            store.read(|s| {
                s.serial_busy()
                    || match key {
                        "batch" | "file" | "macro" | "modbusSend" => {
                            !s.serial_connected()
                                || s.active_session()
                                    .and_then(|s| s.status.as_ref())
                                    .is_some_and(|s| s.job_kind.is_some())
                                || (key == "file" && s.preferences.get("file").is_empty())
                        }
                        "checksum" => s.preferences.get("checksum_input").is_empty(),
                        _ => false,
                    }
            })
        },
        move || feedback.dispatch(store, key),
    )
}
fn json_editor(store: Store, key: &'static str) -> impl IntoElement {
    nsh_base::input()
        .caret_color(|| color(theme().primary))
        .multiline(true)
        .auto_correct(false)
        .auto_capitalize(AutoCapitalize::None)
        .label(field_label(key))
        .value(move || store.field(key))
        .on_change(move |v: &str| store.change(|s| s.set(key, v.into())))
        .h(px(160.))
        .px(px(8.5))
        .py(px(3.5))
        .font_family(mono)
        .text_size(body_size)
        .line_height(px(16.5))
        .text_color(move || {
            theme()
                .color("editor-foreground")
                .unwrap_or(theme().foreground)
        })
        .bg(move || {
            theme()
                .color("editor-background")
                .unwrap_or(theme().background)
        })
        .rounded(px(5.))
        .border(px(0.5))
        .border_color(tool_separator)
}
fn tool_row(label: impl IntoReactive<SharedString>) -> View {
    view()
        .min_h(px(37.))
        .flex_row()
        .items_center()
        .gap_3()
        .border_b_1()
        .border_color(tool_separator)
        .child(text(label).text_size(body_size))
}
fn tool_input(store: Store, key: &'static str, label: Prop<SharedString>) -> View {
    if key == "selected" {
        return view()
            .min_h(px(53.5))
            .pt(px(10.))
            .flex_row()
            .items_start()
            .gap_2()
            .border_b_1()
            .border_color(tool_separator)
            .child(
                text(label)
                    .w(px(269.))
                    .text_size(body_size)
                    .line_height(px(16.25)),
            )
            .child(
                nsh_base::input()
                    .caret_color(|| color(theme().primary))
                    .label(label)
                    .value(move || store.field(key))
                    .on_change(move |v: &str| store.change(|s| s.set(key, v.into())))
                    .h(px(16.))
                    .flex_1()
                    .min_w_0()
                    .text_right()
                    .text_size(body_size),
            );
    }
    tool_row(label).child(
        nsh_base::input()
            .caret_color(|| color(theme().primary))
            .label(label)
            .value(move || {
                if numeric_tool_key(key) {
                    tool_number(&store.field(key))
                } else {
                    store.field(key)
                }
            })
            .on_change(move |v: &str| {
                store.change(|s| {
                    s.set(
                        key,
                        if numeric_tool_key(key) {
                            v.replace(',', "")
                        } else {
                            v.into()
                        },
                    )
                })
            })
            .flex_1()
            .min_w_0()
            .h(px(24.))
            .text_right()
            .text_size(body_size),
    )
}
fn tool_pick(
    store: Store,
    key: &'static str,
    label: Prop<SharedString>,
    values: &'static [&'static str],
) -> View {
    tool_select(
        key,
        label,
        values,
        move || store.field(key),
        move |value| store.change(|s| s.set(key, value)),
    )
}

fn tool_select(
    key: &'static str,
    label: Prop<SharedString>,
    values: &'static [&'static str],
    value: impl Fn() -> String + Copy + 'static,
    change: impl Fn(String) + Copy + 'static,
) -> View {
    tool_row(label)
        .min_h(px(38.25))
        .child(view().flex_1())
        .child(
            nsh_base::select()
                .items(
                    values
                        .iter()
                        .map(|v| nsh_base::SelectItem::new(*v, tool_value_label(key, v)))
                        .collect::<Vec<_>>(),
                )
                .value(move || Some(SharedString::from(value())))
                .on_change(move |v: &Option<SharedString>| {
                    if let Some(v) = v {
                        change(v.to_string())
                    }
                })
                .accessibility_label(move || Some(label.get()))
                .popup_style(popup)
                .render_item(|item: &nsh_base::SelectEntry| {
                    text(item.title.clone()).text_size(body_size).px_2().py_1()
                })
                .trigger(
                    view()
                        .h(px(24.))
                        .flex_row()
                        .items_center()
                        .pr_1()
                        .gap(px(11.))
                        .child(text(move || tool_value_label(key, &value())).text_size(body_size))
                        .child(
                            view()
                                .size(px(20.))
                                .rounded_full()
                                .items_center()
                                .justify_center()
                                .bg(move || {
                                    theme().color("serial-control").unwrap_or(theme().secondary)
                                })
                                .child(icons::icon("select").size(px(16.))),
                        ),
                ),
        )
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PropEnum)]
pub enum ToolParameterId {
    #[default]
    Packets,
    Selected,
    Interval,
    FilePath,
    Chunk,
    Macro,
    Cycles,
    Replies,
    FramingMode,
    FramingValue,
    Station,
    Function,
    Register,
    ModbusValue,
    Timeout,
    RawCapture,
    CaptureFormat,
    CapturePath,
    RollMib,
    ChecksumInput,
    Format,
    Checksum,
    Prefix,
    Suffix,
    HexEnding,
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct ToolParameterProps {
    /// Serial field or draft represented by this control.
    pub field: Prop<ToolParameterId>,
    /// Label visible in the row and announced by the editor.
    pub label: Prop<SharedString>,
}

/// A native editor or picker bound to the captured tool session.
#[component]
pub fn ToolParameter(props: ToolParameterProps) -> impl IntoElement {
    use ToolParameterId::*;
    let ToolSession {
        store,
        packets,
        framing_mode,
        framing_value,
        feedback,
    } = ToolSession::current();
    let label = props.label;
    match props.field.get() {
        Packets => nsh_base::input()
            .caret_color(|| color(theme().primary))
            .multiline(true)
            .auto_correct(false)
            .auto_capitalize(AutoCapitalize::None)
            .label(label)
            .auto_focus(true)
            .value(packets)
            .on_change(move |v: &str| packets.set(v.into()))
            .h(px(160.))
            .px(px(8.5))
            .py(px(3.5))
            .font_family(mono)
            .text_size(body_size)
            .line_height(px(16.5))
            .text_color(move || {
                theme()
                    .color("editor-foreground")
                    .unwrap_or(theme().foreground)
            })
            .bg(move || {
                theme()
                    .color("editor-background")
                    .unwrap_or(theme().background)
            })
            .rounded(px(5.))
            .border(px(0.5))
            .border_color(tool_separator)
            .into_element(),
        Macro => json_editor(store, "macro").into_element(),
        Replies => json_editor(store, "replies").into_element(),
        Selected => tool_input(store, "selected", label).into_element(),
        Interval => tool_input(store, "interval", label).into_element(),
        Chunk => tool_input(store, "chunk", label).into_element(),
        Cycles => tool_input(store, "cycles", label).into_element(),
        Station => tool_input(store, "station", label).into_element(),
        Register => tool_input(store, "register", label).into_element(),
        Timeout => tool_input(store, "timeout", label).into_element(),
        RollMib => tool_input(store, "roll_mib", label).into_element(),
        Prefix => tool_input(store, "prefix", label).into_element(),
        Suffix => tool_input(store, "suffix", label).into_element(),
        HexEnding => tool_toggle(store, "hex_ending", label).into_element(),
        RawCapture => tool_toggle(store, "raw_capture", label).into_element(),
        Format => tool_pick(
            store,
            "format",
            label,
            &["hex", "utf-8", "ascii", "gbk", "latin-1"],
        )
        .into_element(),
        Checksum => tool_pick(
            store,
            "checksum",
            label,
            &[
                "sum8",
                "xor8",
                "oddParity",
                "evenParity",
                "crc16Modbus",
                "crc16Ccitt",
                "crc32",
                "lrc",
            ],
        )
        .into_element(),
        Function => tool_pick(
            store,
            "function",
            label,
            &["1", "2", "3", "4", "5", "6", "15", "16"],
        )
        .into_element(),
        ModbusValue => when(
            move || ["1", "2", "3", "4"].contains(&store.field("function").as_str()),
            move || tool_input(store, "quantity", "读取数量".into()),
        )
        .otherwise(move || tool_input(store, "values", "写入值（十进制，以逗号分隔）".into()))
        .into_element(),
        FramingMode => tool_select(
            "frame_mode",
            label,
            &["read", "idle", "header", "tail", "fixed"],
            move || framing_mode.get(),
            move |v| framing_mode.set(v),
        )
        .into_element(),
        FramingValue => when(
            move || framing_mode.get() != "read",
            move || {
                dynamic(move || {
                    let label = match framing_mode.get().as_str() {
                        "idle" => "间隔 (ms)",
                        "fixed" => "字节数",
                        _ => "标记 (HEX)",
                    };
                    tool_row(label).child(
                        nsh_base::input()
                            .caret_color(|| color(theme().primary))
                            .label(label)
                            .value(framing_value)
                            .on_change(move |v: &str| framing_value.set(v.into()))
                            .flex_1()
                            .min_w_0()
                            .h(px(24.))
                            .text_right()
                            .text_size(body_size),
                    )
                })
            },
        )
        .into_element(),
        CaptureFormat => when(
            move || !store.flag("raw_capture"),
            move || tool_pick(store, "capture_format", label, &["csv", "txt", "bin"]),
        )
        .into_element(),
        FilePath => tool_row(label)
            .min_h(px(45.))
            .child(
                nsh_base::input()
                    .caret_color(|| color(theme().primary))
                    .label(label)
                    .value(move || store.field("file"))
                    .on_change(move |v: &str| store.change(|s| s.set("file", v.into())))
                    .flex_1()
                    .min_w_0()
                    .text_size(body_size),
            )
            .child(tool_action(
                "选择文件…",
                feedback,
                || false,
                move || pick_file(store),
            ))
            .into_element(),
        CapturePath => view()
            .min_h(px(45.))
            .flex_row()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(tool_separator)
            .child(text(label).text_size(body_size))
            .child(
                nsh_base::input()
                    .caret_color(|| color(theme().primary))
                    .auto_correct(false)
                    .auto_capitalize(AutoCapitalize::None)
                    .label(label)
                    .value(move || store.field("capture_path"))
                    .on_change(move |v: &str| store.change(|s| s.set("capture_path", v.into())))
                    .flex_1()
                    .text_size(body_size),
            )
            .child(tool_action(
                "选择文件夹",
                feedback,
                || false,
                move || capture_directory(store),
            ))
            .into_element(),
        ChecksumInput => view()
            .min_h(px(53.))
            .flex_row()
            .items_start()
            .pt(px(10.))
            .gap_3()
            .border_b_1()
            .border_color(tool_separator)
            .child(text(label).text_size(body_size))
            .child(
                nsh_base::input()
                    .caret_color(|| color(theme().primary))
                    .multiline(true)
                    .auto_correct(false)
                    .auto_capitalize(AutoCapitalize::None)
                    .label(label)
                    .auto_focus(true)
                    .text_right()
                    .value(move || store.field("checksum_input"))
                    .on_change(move |v: &str| store.change(|s| s.set("checksum_input", v.into())))
                    .h(px(40.))
                    .flex_1()
                    .text_size(body_size),
            )
            .into_element(),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PropEnum)]
pub enum ToolCommand {
    #[default]
    Checksum,
    Framing,
    Modbus,
    ModbusSend,
    Capture,
    File,
    Macro,
    MacroExport,
    Replies,
    RepliesExample,
    PacketsExample,
    PacketsImport,
    PacketsSave,
    PacketsExport,
    Batch,
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct ToolCommandButtonProps {
    /// The operation to perform on the tool session.
    pub command: Prop<ToolCommand>,
    /// Action label.
    pub label: Prop<SharedString>,
}

/// A native action using the same availability guards as the workbench.
#[component]
pub fn ToolCommandButton(props: ToolCommandButtonProps) -> impl IntoElement {
    use ToolCommand::*;
    let ToolSession {
        store,
        packets,
        framing_mode,
        framing_value,
        feedback,
    } = ToolSession::current();
    let label = props.label;
    match props.command.get() {
        Checksum => run(store,"checksum",label,feedback).into_element(),
        Modbus => run(store,"modbus",label,feedback).into_element(),
        ModbusSend => run(store,"modbusSend",label,feedback).into_element(),
        File => run(store,"file",label,feedback).into_element(),
        Macro => run(store,"macro",label,feedback).into_element(),
        Replies => run(store,"replies",label,feedback).into_element(),
        Framing => tool_action(label,feedback,move ||store.read(Workbench::serial_busy),move ||feedback.dispatch_framing(store,framing_mode.get_untracked(),framing_value.get_untracked())).into_element(),
        Capture => tool_action(move ||if capturing(store){SharedString::from("停止采集")}else{label.get()},feedback,move ||store.read(Workbench::serial_busy)||(!capturing(store)&&store.field("capture_path").is_empty()),move || {
            let raw=store.flag("raw_capture");let running=capturing(store);feedback.dispatch(store,match(raw,running){(true,true)=>"stopRaw",(true,false)=>"raw",(false,true)=>"stopCapture",_=>"capture"});
        }).into_element(),
        MacroExport => tool_action(label,feedback,||false,move ||save_macro(store,feedback)).into_element(),
        RepliesExample => tool_action(label,feedback,||false,move ||store.change(|s|s.set("replies",r#"[{"name":"pong","enabled":true,"trigger":{"kind":"text","pattern":"PING"},"reply":{"text":"OK","format":"utf-8"},"delayMs":20}]"#.into()))).into_element(),
        PacketsExample => tool_action(label,feedback,||false,move ||packets.set(r#"[{"name":"读状态","group":"调试","note":"","order":0,"enabled":true,"text":"AA 01","format":"hex"}]"#.into())).into_element(),
        PacketsImport => tool_action(label,feedback,||false,move ||import_packets(store,packets,feedback)).into_element(),
        PacketsSave => tool_action(label,feedback,||false,move ||save_packets(store,packets,false,feedback)).into_element(),
        PacketsExport => tool_action(label,feedback,||false,move ||export_packets(store,feedback)).into_element(),
        Batch => tool_action(label,feedback,move ||store.read(|s|s.serial_busy()||!s.serial_connected()),move ||save_packets(store,packets,true,feedback)).into_element(),
    }
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct ToolOutcomeProps {
    /// Show the diagnostic GroupBox rather than a workflow label.
    #[prop(default)]
    pub diagnostic: Prop<bool>,
    /// Present the current error.
    #[prop(default)]
    pub failed: Prop<bool>,
}
/// The result or error bound to the captured operation revision.
#[component]
pub fn ToolOutcome(props: ToolOutcomeProps) -> impl IntoElement {
    let feedback = ToolSession::current().feedback;
    if props.diagnostic.get() {
        feedback::result_box(feedback)
    } else {
        feedback::message(feedback, props.failed.get())
    }
}
/// The selected serial session's native job progress and cancellation control.
#[component]
pub fn SerialJobStatus() -> impl IntoElement {
    let state = ToolSession::current();
    feedback::job(state.store, state.feedback)
}
/// Closes the active dialog using its native button and keyboard semantics.
#[component]
pub fn ToolDone() -> impl IntoElement {
    let store = ToolSession::current().store;
    action("完成", Variant::Default, || false, move || close(store))
}

fn capturing(store: Store) -> bool {
    store.read(|s| {
        s.active_session()
            .and_then(|s| s.status.as_ref())
            .is_some_and(|v| {
                if s.preferences.flag("raw_capture") {
                    v.capturing
                } else {
                    v.logging
                }
            })
    })
}
fn pick_file(store: Store) {
    if store.file_dialog.get_untracked() {
        return;
    }
    let session = store.tool_session.get_untracked();
    store.file_dialog.set(true);
    store.spawn(async move {
        let result = super::file_dialogs::open_path(super::file_dialogs::OpenKind::File).await;
        store.file_dialog.set(false);
        if store.tool_session.get_untracked() != session || store.read(|s| s.active) != session {
            return;
        }
        match result {
            Ok(Some(path)) => store.change(|s| s.set("file", path.to_string_lossy().into_owned())),
            Ok(None) => {}
            Err(error) => store.report(error),
        }
    });
}
fn capture_directory(store: Store) {
    if store.file_dialog.get_untracked() {
        return;
    }
    let Some(session) = store
        .tool_session
        .get_untracked()
        .or(store.read(|s| s.active))
    else {
        return;
    };
    let format = if store.flag("raw_capture") {
        "bin".to_owned()
    } else {
        store.field("capture_format")
    };
    store.file_dialog.set(true);
    store.spawn(async move {
        let result = super::file_dialogs::open_path(super::file_dialogs::OpenKind::Folder).await;
        store.file_dialog.set(false);
        if store.tool_session.get_untracked() != Some(session)
            || store.read(|s| s.active) != Some(session)
        {
            return;
        }
        match result {
            Ok(Some(path)) => {
                let name = format!(
                    "ChipDock-{session}-{}.{}",
                    chrono::Local::now().timestamp(),
                    format
                );
                store.change(|s| {
                    s.set(
                        "capture_path",
                        path.join(name).to_string_lossy().into_owned(),
                    )
                });
            }
            Ok(None) => {}
            Err(e) => store.report(e),
        }
    });
}
fn save_macro(store: Store, feedback: Feedback) {
    if store.file_dialog.get_untracked() {
        return;
    }
    let Some(session) = store
        .tool_session
        .get_untracked()
        .or(store.read(|s| s.active))
    else {
        return;
    };
    store.file_dialog.set(true);
    store.spawn(async move {
        let result = super::file_dialogs::save_path(
            "ChipDock-Macro.csv".into(),
            super::file_dialogs::ExportKind::Save,
        )
        .await;
        store.file_dialog.set(false);
        match result {
            Ok(Some(path)) => {
                if store.read(|s| s.sessions.contains_key(&session)) {
                    let request =
                        serde_json::json!({"action":"exportMacro", "path":path.to_string_lossy()});
                    if feedback.running.try_get().is_some() {
                        feedback.dispatch_request(store, "exportMacro", request);
                    } else {
                        store.command(move |_| Command::Workbench(session, request));
                    }
                }
            }
            Ok(None) => {}
            Err(e) => feedback.finish(Err(e)),
        }
    });
}
#[derive(Clone, Copy)]
struct ProfileDraft {
    store: Store,
    name: Signal<String>,
}

impl ProfileDraft {
    fn current() -> Self {
        use_context().expect("profile controls require ProfileSession")
    }

    fn save(self) {
        if self.name.with_untracked(|name| name.trim().is_empty()) {
            return;
        }
        let result = self.store.change(|s| {
            let old_name = s.preferences.get("profile").to_owned();
            let profiles = s.preferences.profiles.clone();
            let requested_name = self.name.get_untracked();
            let stored_name = s.preferences.unique_profile_name(requested_name.trim());
            s.set("profile", requested_name);
            match s.store_profile() {
                Ok(()) => Ok(stored_name),
                Err(error) => {
                    s.preferences.profiles = profiles;
                    s.set("profile", old_name);
                    Err(error)
                }
            }
        });
        match result {
            Ok(stored_name) => {
                self.store
                    .change(|s| s.report(format!("已保存配置“{stored_name}”")));
                close(self.store);
            }
            Err(error) => self.store.queue_storage_error(error),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ProfileSessionProps {
    /// The profile editor's fields and actions.
    pub children: Prop<Children>,
}
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ProfileSessionEvent {
    /// The target and connection parameters included when saving this profile.
    #[args(target, wire, speed, erase)]
    SummaryChange(String, String, String, String),
}
/// Owns the editable name and exposes the current profile summary to TSX.
#[component]
pub fn ProfileSession(
    props: ProfileSessionProps,
    cx: Scope<ProfileSessionEvent>,
) -> impl IntoElement {
    let (store, _) = workbench_context();
    provide_context(components::ModalControls);
    let target = store.field("chip");
    let name = signal(if target.is_empty() {
        "烧录配置".to_owned()
    } else {
        target
    });
    provide_context(ProfileDraft { store, name });
    let summary = computed(move || {
        store.read(|s| {
            let p = &s.preferences;
            (
                p.get("chip").to_owned(),
                p.get("wire").to_uppercase(),
                p.get("speed").to_owned(),
                p.get("erase").to_owned(),
            )
        })
    });
    effect(move || {
        let (target, wire, speed, erase) = summary.get();
        cx.emit(ProfileSessionEvent::SummaryChange(
            target, wire, speed, erase,
        ));
    });
    view().child(props.children)
}
/// Profile name with native initial selection and Return-to-save behavior.
#[component]
pub fn ProfileName() -> impl IntoElement {
    let draft = ProfileDraft::current();
    let selected = draft.name.get_untracked().encode_utf16().count() as u32;
    let selection = signal((0, 0));
    let first_focus = signal(true);
    draft_input(
        "配置名称",
        move || draft.name.get(),
        move |v| draft.name.set(v.into()),
    )
    .placeholder("配置名称")
    .auto_correct(false)
    .auto_capitalize(AutoCapitalize::None)
    .auto_focus(true)
    .selected_range(selection)
    .on_focus(move || {
        if first_focus.get_untracked() {
            first_focus.set(false);
            selection.set((0, selected));
        }
    })
    .on_submit(move |_: &str| draft.save())
    .focus(|s| {
        s.outline(px(3.5))
            .outline_color(move || theme().primary.opacity(0.5))
    })
}
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ProfileActionProps {
    /// Save as the default action, or cancel and discard the draft.
    #[prop(default)]
    pub save: Prop<bool>,
}
/// Commits or dismisses the profile using the workbench's native buttons.
#[component]
pub fn ProfileAction(props: ProfileActionProps) -> impl IntoElement {
    let draft = ProfileDraft::current();
    if props.save.get() {
        action(
            "保存",
            Variant::DefaultAction,
            move || draft.name.with(|s| s.trim().is_empty()),
            move || draft.save(),
        )
        .into_element()
    } else {
        action(
            "取消",
            Variant::Default,
            || false,
            move || close(draft.store),
        )
        .into_element()
    }
}

pub(super) async fn read_json(
    kind: super::file_dialogs::OpenKind,
) -> Result<Option<Value>, String> {
    read_json_source(kind)
        .await?
        .map(|source| serde_json::from_str(&source).map_err(|error| format!("{error:#}")))
        .transpose()
}

async fn read_json_source(kind: super::file_dialogs::OpenKind) -> Result<Option<String>, String> {
    let Some(path) = super::file_dialogs::open_path(kind).await? else {
        return Ok(None);
    };
    nsh_core::element::blocking(move || -> anyhow::Result<String> {
        use std::io::Read;
        let mut source = String::new();
        std::fs::File::open(path)?
            .take(1024 * 1024 + 1)
            .read_to_string(&mut source)?;
        anyhow::ensure!(source.len() <= 1024 * 1024, "配置超过 1 MiB");
        Ok(source)
    })
    .await
    .map_err(|error| error.to_string())?
    .map(Some)
    .map_err(|error| format!("{error:#}"))
}
fn tool_value_label(key: &str, value: &str) -> String {
    match key {
        "frame_mode" => match value {
            "read" => "读取块",
            "idle" => "静默间隔",
            "header" => "帧头",
            "tail" => "帧尾",
            "fixed" => "固定长度",
            _ => value,
        }
        .into(),
        "function" => value
            .parse::<u8>()
            .map(|value| format!("{value:02X}"))
            .unwrap_or_else(|_| value.into()),
        "capture_format" => value.to_uppercase(),
        _ => value_label(key, value),
    }
}

fn tool_toggle(store: Store, key: &'static str, label: Prop<SharedString>) -> View {
    tool_row(label)
        .min_h(px(37.5))
        .child(view().flex_1())
        .child(
            nsh_base::switch()
                .checked(move || store.flag(key))
                .on_change(move |v: &bool| store.change(|s| s.flag(key, *v)))
                .accessibility_label(move || Some(label.get()))
                .track_style(|s| {
                    s.w(px(36.))
                        .h(px(16.))
                        .p(px(2.))
                        .rounded_full()
                        .bg(move || {
                            theme()
                                .color("tools-switch-off")
                                .unwrap_or(theme().secondary)
                        })
                        .checked(|s| s.bg(move || theme().primary))
                        .transition_colors()
                        .duration(INSTANT)
                })
                .thumb_style(|s| {
                    s.w(px(22.))
                        .h(px(12.))
                        .rounded_full()
                        .bg(move || theme().primary_foreground)
                        .checked(|s| s.translate_x(px(10.)))
                        .transition_transform()
                        .duration(FAST)
                }),
        )
}

fn import_packets(store: Store, draft: Signal<String>, feedback: Feedback) {
    if store.file_dialog.get_untracked() {
        return;
    }
    store.file_dialog.set(true);
    store.spawn(async move {
        let result = read_json_source(super::file_dialogs::OpenKind::Json).await;
        store.file_dialog.set(false);
        if draft.try_get().is_none() {
            return;
        }
        match result {
            Ok(Some(source)) => {
                commit_packets(store, draft, source, PacketCommit::Import, feedback)
            }
            Ok(None) => {}
            Err(error) => feedback.finish(Err(error)),
        }
    });
}

fn save_packets(store: Store, draft: Signal<String>, send: bool, feedback: Feedback) {
    commit_packets(
        store,
        draft,
        draft.get_untracked(),
        if send {
            PacketCommit::Batch
        } else {
            PacketCommit::Save
        },
        feedback,
    );
}

#[derive(Clone, Copy)]
enum PacketCommit {
    Save,
    Import,
    Batch,
}

fn commit_packets(
    store: Store,
    draft: Signal<String>,
    source: String,
    action: PacketCommit,
    feedback: Feedback,
) {
    let Some(id) = feedback.session else {
        feedback.finish(Err("串口会话已关闭".into()));
        return;
    };
    let original_draft = draft.get_untracked();
    let generation = feedback.begin(store);
    store.spawn(async move {
        let validating = source.clone();
        let result =
            nsh_core::element::blocking(move || validate_packets_source(id, &validating)).await;
        if !feedback.current(generation) {
            return;
        }
        if store.read(|s| s.active) != Some(id)
            || store.tool_session.get_untracked() != Some(id)
            || draft.try_get().as_deref() != Some(original_draft.as_str())
        {
            feedback.finish(Err("包库内容或会话已改变，未保存".into()));
            return;
        }
        match result {
            Ok(Ok(_)) => {
                let saved = store.change(|s| {
                    let previous = s.preferences.get("packets").to_owned();
                    s.set("packets", source.clone());
                    match s.save() {
                        Ok(()) => Ok(()),
                        Err(error) => {
                            s.set("packets", previous);
                            Err(error)
                        }
                    }
                });
                match saved {
                    Ok(()) => {
                        if matches!(action, PacketCommit::Import) {
                            draft.set(source);
                        }
                        if matches!(action, PacketCommit::Batch) {
                            feedback.dispatch(store, "batch");
                        } else {
                            feedback.finish(Ok(if matches!(action, PacketCommit::Import) {
                                "已导入包库"
                            } else {
                                "已保存包库"
                            }
                            .into()));
                        }
                    }
                    Err(error) => feedback.finish(Err(format!("保存包库失败：{error}"))),
                }
            }
            Ok(Err(error)) => feedback.finish(Err(error)),
            Err(error) => feedback.finish(Err(error.to_string())),
        }
    });
}

fn export_packets(store: Store, feedback: Feedback) {
    if store.file_dialog.get_untracked() {
        return;
    }
    let bytes = store.field("packets").into_bytes();
    store.file_dialog.set(true);
    store.spawn(async move {
        let chosen = super::file_dialogs::save_path(
            "ChipDock-Packets.json".into(),
            super::file_dialogs::ExportKind::Save,
        )
        .await;
        store.file_dialog.set(false);
        match chosen {
            Ok(Some(path)) => {
                if !store.change(Workbench::begin_file_write) {
                    feedback.finish(Err("更新安装期间无法导出".into()));
                    return;
                }
                let generation = if feedback.running.try_get().is_some() {
                    Some(feedback.begin(store))
                } else {
                    None
                };
                let result =
                    nsh_core::element::blocking(move || crate::model::write_atomic(&path, &bytes))
                        .await;
                store.change(Workbench::finish_file_write);
                if generation.is_some_and(|generation| feedback.current(generation)) {
                    feedback.finish(match result {
                        Ok(Ok(())) => Ok("已导出".into()),
                        Ok(Err(error)) => Err(format!("{error:#}")),
                        Err(error) => Err(error.to_string()),
                    });
                }
            }
            Ok(None) => {}
            Err(error) => feedback.finish(Err(error)),
        }
    });
}
fn validate_packets_source(id: u64, source: &str) -> Result<String, String> {
    if source.len() > 1024 * 1024 {
        return Err("包库超过 1 MiB".into());
    }
    let packets: Value =
        serde_json::from_str(source).map_err(|error| format!("包库 JSON 无效：{error}"))?;
    chipdock_service::serial::serial_workbench(
        id,
        &serde_json::json!({"action":"validatePackets","packets":packets}).to_string(),
    )
    .map_err(|error| error.to_string())
}

fn tool_number(source: &str) -> String {
    if !source.bytes().all(|b| b.is_ascii_digit()) {
        return source.into();
    }
    let digits = source.as_bytes();
    let mut text = String::new();
    for (index, byte) in digits.iter().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            text.push(',');
        }
        text.push(*byte as char);
    }
    text
}

fn numeric_tool_key(key: &str) -> bool {
    matches!(
        key,
        "station"
            | "register"
            | "quantity"
            | "timeout"
            | "chunk"
            | "interval"
            | "cycles"
            | "prefix"
            | "suffix"
            | "roll_mib"
    )
}

// NSGroupedForm separator contrast measured against the native dark and light captures.
fn tool_separator() -> AdaptiveColor {
    theme().foreground.opacity(0.055)
}

#[cfg(test)]
mod packet_save_tests {
    use super::{numeric_tool_key, tool_number, validate_packets_source};
    #[test]
    fn native_numeric_display_keeps_non_numeric_drafts_and_packet_name_lists() {
        assert_eq!(tool_number("4096"), "4,096");
        assert_eq!(tool_number("1000"), "1,000");
        assert_eq!(tool_number("0x1000"), "0x1000");
        assert_eq!(tool_number("16.5"), "16.5");
        assert!(!numeric_tool_key("selected"));
    }
    #[test]
    fn invalid_packet_content_is_rejected_without_a_device_connection() {
        let id = chipdock_service::serial::serial_create_session().unwrap();
        let result = validate_packets_source(id, r#"[{"name":"bad","text":"GG","format":"hex"}]"#);
        chipdock_service::serial::serial_close_session(id).unwrap();
        assert!(
            result.is_err(),
            "invalid HEX must not reach the save success branch"
        );
    }
    #[test]
    fn valid_packet_configuration_can_be_saved_while_disconnected() {
        let id = chipdock_service::serial::serial_create_session().unwrap();
        let result =
            validate_packets_source(id, r#"[{"name":"PING","text":"PING","format":"utf-8"}]"#);
        chipdock_service::serial::serial_close_session(id).unwrap();
        assert!(result.is_ok());
    }
}
