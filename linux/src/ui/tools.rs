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
pub(super) fn overlay(store: Store) -> impl IntoElement {
    dynamic(move || {
        let Some(key) = store.tool.get() else {
            return view().into_element();
        };
        untracked(move || {
            provide_context(components::ModalControls);
            let content = match key {
                "protocol" => super::protocol::page(store).into_element(),
                "colors" => super::colors::page(store).into_element(),
                "records" => super::records::page(store).into_element(),
                "profile" => profile(store).into_element(),
                _ => grouped(store, key).into_element(),
            };
            nsh_base::dialog()
                .on_cancel(move || close(store))
                .flex_1()
                .items_center()
                .justify_center()
                .bg(move || modal_backdrop(store))
                .text_color(move || theme().foreground)
                .text_size(body_size)
                .line_height(nsh_core::LineHeight::Multiple(1.2))
                .child(
                    view()
                        .bg(move || theme().background)
                        .rounded(px(16.))
                        .overflow_hidden()
                        .child(content),
                )
                .into_element()
        })
    })
}
#[derive(Clone, Copy)]
struct FramingDraft {
    mode: Signal<String>,
    value: Signal<String>,
}

fn grouped(store: Store, key: &'static str) -> View {
    let workflow = matches!(key, "packets" | "file" | "macro" | "replies");
    let page = signal(key);
    let packets_draft = signal(store.field("packets"));
    let framing = FramingDraft {
        mode: signal(store.field("frame_mode")),
        value: signal(store.field("frame_value")),
    };
    let feedback = Feedback::new(store);
    effect(move || {
        page.get();
        untracked(move || feedback.reset(store));
    });
    view()
        .w(px(if workflow { 660. } else { 540. }))
        .h(move || {
            let base = match page.get() {
                "checksum" => 456.,
                "packets" => 505.,
                "file" => 392.,
                "macro" => 451.,
                "replies" => 414.,
                _ => 411.,
            };
            let has_result = !feedback.result.get().is_empty();
            let has_error = !feedback.error.get().is_empty();
            let job = if workflow {
                feedback::job_height(store, feedback)
            } else {
                0.
            };
            px(if workflow && (has_result || has_error) {
                base + job + 12. + feedback.label_height.get()
            } else if !workflow && has_result {
                562.
            } else if !workflow && has_error && page.get() != "capture" {
                411. + 14. + feedback.label_height.get()
            } else {
                base + job
            })
        })
        .p_5()
        .gap(px(if workflow { 12. } else { 14. }))
        .child(tool_tabs(page, workflow))
        .child((!workflow).then(|| view().h(px(1.)).bg(hairline)))
        .child(dynamic(move || {
            let key = page.get();
            untracked(move || {
                scroll().flex_1().min_h_0().child(
                    view()
                        .p_5()
                        .child(form(store, key, packets_draft, framing, feedback).key(key)),
                )
            })
        }))
        .child(workflow.then(|| feedback::job(store, feedback)))
        .child(when(
            move || !feedback.result.get().is_empty(),
            move || {
                if workflow {
                    feedback::message(feedback, false)
                } else {
                    feedback::result_box(feedback)
                }
            },
        ))
        .child(when(
            move || !feedback.error.get().is_empty() && page.get() != "capture",
            move || feedback::message(feedback, true),
        ))
        .child(view().flex_row().justify_end().child(action(
            "完成",
            Variant::Default,
            || false,
            move || close(store),
        )))
}
fn tool_tabs(page: Signal<&'static str>, workflow: bool) -> impl IntoElement {
    let entries = if workflow {
        &[
            ("packets", "常用包库"),
            ("file", "发送文件"),
            ("macro", "宏命令"),
            ("replies", "自动应答"),
        ][..]
    } else {
        &[
            ("checksum", "发送校验"),
            ("framing", "接收分帧"),
            ("modbus", "Modbus RTU"),
            ("capture", "持续采集"),
        ][..]
    };
    nsh_base::tabs()
        .self_start()
        .h(px(24.))
        .p_0()
        .flex_row()
        .rounded(px(7.))
        .bg(move || theme().color("tools-tab").unwrap_or(theme().secondary))
        .indicator_style(|s| {
            s.top_0()
                .bottom_0()
                .rounded(px(6.))
                .bg(move || theme().primary)
                .transition_transform()
                .duration(BASE)
        })
        .children(
            entries
                .iter()
                .enumerate()
                .map(move |(index, (key, label))| {
                    nsh_base::tab()
                        .relative()
                        .selected(move || page.get() == *key)
                        .on_press(move || page.set(key))
                        .w(px(if workflow { 76. } else { 103. }))
                        .h(px(24.))
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .selected_style(|s| s.text_color(move || theme().primary_foreground))
                        .child(when(
                            move || {
                                index > 0
                                    && page.get() != *key
                                    && page.get() != entries[index - 1].0
                            },
                            move || {
                                view()
                                    .absolute()
                                    .left_0()
                                    .top(px(5.))
                                    .w(px(1.))
                                    .h(px(14.))
                                    .bg(move || {
                                        theme().color("tools-tab-divider").unwrap_or(theme().border)
                                    })
                            },
                        ))
                        .child(text(*label).text_size(body_size))
                }),
        )
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
    label: &'static str,
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
fn tool_row(label: &'static str) -> View {
    view()
        .min_h(px(37.))
        .flex_row()
        .items_center()
        .gap_3()
        .border_b_1()
        .border_color(tool_separator)
        .child(text(label).text_size(body_size))
}
fn tool_input(store: Store, key: &'static str, label: &'static str) -> View {
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
    label: &'static str,
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
    label: &'static str,
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
                .accessibility_label(label)
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
fn form(
    store: Store,
    key: &'static str,
    packets_draft: Signal<String>,
    framing: FramingDraft,
    feedback: Feedback,
) -> View {
    match key {
        "packets"=>form_group()
            .child(view().py_3().border_b_1().border_color(tool_separator).child(tool_note("每个包可设置 name、group、note、order、enabled、text、format。format 使用 hex 或 utf-8。")))
            .child(view().py(px(10.)).border_b_1().border_color(tool_separator).child(nsh_base::input().caret_color(|| color(theme().primary)).multiline(true).auto_correct(false).auto_capitalize(AutoCapitalize::None).label("常用包库").auto_focus(true).value(packets_draft).on_change(move |v:&str|packets_draft.set(v.into())).h(px(160.)).px(px(8.5)).py(px(3.5)).font_family(mono).text_size(body_size).line_height(px(16.5)).text_color(move ||theme().color("editor-foreground").unwrap_or(theme().foreground)).bg(move ||theme().color("editor-background").unwrap_or(theme().background)).rounded(px(5.)).border(px(0.5)).border_color(tool_separator)))
            .child(tool_input(store,"selected","按发送顺序填写包名（逗号分隔，留空发送全部启用包）"))
            .child(tool_input(store,"interval","包间隔 (ms)"))
            .child(row().py(px(10.)).child(tool_action("示例",feedback,||false,move ||packets_draft.set(r#"[{"name":"读状态","group":"调试","note":"","order":0,"enabled":true,"text":"AA 01","format":"hex"}]"#.into())))
                .child(tool_action("导入 JSON…",feedback,||false,move ||import_packets(store,packets_draft,feedback)))
                .child(tool_action("保存",feedback,||false,move ||save_packets(store,packets_draft,false,feedback)))
                .child(tool_action("导出 JSON…",feedback,||false,move ||export_packets(store,feedback)))
                .child(tool_action("批量发送",feedback,move ||store.read(|s|s.serial_busy()||!s.serial_connected()),move ||save_packets(store,packets_draft,true,feedback)))),
        "file"=>form_group().child(tool_row("文件路径").min_h(px(45.)).child(nsh_base::input().caret_color(|| color(theme().primary)).label("文件路径").value(move ||store.field("file")).on_change(move |v:&str|store.change(|s|s.set("file",v.into()))).flex_1().min_w_0().text_size(body_size))
            .child(tool_action("选择文件…",feedback,||false,move ||pick_file(store))))
            .child(tool_input(store,"chunk","块大小 (字节)")).child(tool_input(store,"interval","块间隔 (ms)"))
            .child(view().py(px(10.)).items_start().child(run(store,"file","发送文件",feedback))),
        "macro"=>form_group().child(view().py_3().border_b_1().border_color(tool_separator).child(tool_note("步骤可包含 send、delayMs、wait 和 timeoutMs。匹配方式为 text、hex 或 regex。")))
            .child(view().py(px(10.)).border_b_1().border_color(tool_separator).child(json_editor(store,"macro")))
            .child(tool_input(store,"cycles","循环次数"))
            .child(row().py(px(10.)).child(run(store,"macro","运行宏",feedback)).child(tool_action("导出结果 CSV…",feedback,||false,move ||save_macro(store,feedback)))),
        "replies"=>form_group().child(view().py_3().border_b_1().border_color(tool_separator).child(tool_note("按顺序匹配，支持 text、hex 或 regex。delayMs 用于延时，enabled 控制是否启用。")))
            .child(view().py(px(10.)).border_b_1().border_color(tool_separator).child(json_editor(store,"replies")))
            .child(row().py(px(10.)).child(tool_action("示例",feedback,||false,move ||store.change(|s|s.set("replies",r#"[{"name":"pong","enabled":true,"trigger":{"kind":"text","pattern":"PING"},"reply":{"text":"OK","format":"utf-8"},"delayMs":20}]"#.into()))))
                .child(run(store,"replies","应用应答规则",feedback))),
        "framing"=>form_group().child(tool_select("frame_mode","分帧方式",&["read","idle","header","tail","fixed"],move ||framing.mode.get(),move |value|framing.mode.set(value)))
            .child(when(move ||framing.mode.get()!="read",move ||dynamic(move ||{let label=match framing.mode.get().as_str(){"idle"=>"间隔 (ms)","fixed"=>"字节数",_=>"标记 (HEX)"};tool_row(label).child(nsh_base::input().caret_color(||color(theme().primary)).label(label).value(framing.value).on_change(move |value:&str|framing.value.set(value.into())).flex_1().min_w_0().h(px(24.)).text_right().text_size(body_size))})))
            .child(view().py(px(10.)).items_start().child(tool_action("应用",feedback,move ||store.read(Workbench::serial_busy),move ||feedback.dispatch_framing(store,framing.mode.get_untracked(),framing.value.get_untracked())))),
        "modbus"=>form_group().child(tool_input(store,"station","站号")).child(tool_pick(store,"function","功能码",&["1","2","3","4","5","6","15","16"]))
            .child(tool_input(store,"register","起始地址")).child(when(move ||["1","2","3","4"].contains(&store.field("function").as_str()),move ||tool_input(store,"quantity","读取数量")).otherwise(move ||tool_input(store,"values","写入值（十进制，以逗号分隔）")))
            .child(tool_input(store,"timeout","超时 (ms)"))
            .child(row().py(px(10.)).child(run(store,"modbus","生成请求",feedback)).child(run(store,"modbusSend","发送并等待响应",feedback))),
        "capture"=>form_group().child(tool_toggle(store,"raw_capture","仅捕获原始 RX"))
            .child(when(move ||!store.flag("raw_capture"),move ||tool_pick(store,"capture_format","日志格式",&["csv","txt","bin"])))
            .child(view().min_h(px(45.)).flex_row().gap_2().items_center().border_b_1().border_color(tool_separator).child(text("新文件路径").text_size(body_size)).child(nsh_base::input().caret_color(|| color(theme().primary)).auto_correct(false).auto_capitalize(AutoCapitalize::None).label("新文件路径").value(move ||store.field("capture_path")).on_change(move |v:&str|store.change(|s|s.set("capture_path",v.into()))).flex_1().text_size(body_size))
                .child(tool_action("选择文件夹",feedback,||false,move ||capture_directory(store))))
            .child(tool_input(store,"roll_mib","按大小滚动 (MiB)"))
            .child(view().py(px(10.)).items_start().child(tool_action(move ||if capturing(store){"停止采集"}else{"开始采集"},feedback,move ||store.read(Workbench::serial_busy)||(!capturing(store)&&store.field("capture_path").is_empty()),move ||{
                let raw=store.flag("raw_capture");let running=capturing(store);feedback.dispatch(store,match(raw,running){(true,true)=>"stopRaw",(true,false)=>"raw",(false,true)=>"stopCapture",_=>"capture"});
            })))
            .child(when(move ||!feedback.error.get().is_empty(),move ||view().py(px(10.)).child(feedback::message(feedback,true)))),
        _=>form_group().child(view().min_h(px(53.)).flex_row().items_start().pt(px(10.)).gap_3().border_b_1().border_color(tool_separator).child(text("试算内容").text_size(body_size))
            .child(nsh_base::input().caret_color(|| color(theme().primary)).multiline(true).auto_correct(false).auto_capitalize(AutoCapitalize::None).label("试算内容").auto_focus(true).text_right().value(move ||store.field("checksum_input")).on_change(move |v:&str|store.change(|s|s.set("checksum_input",v.into()))).h(px(40.)).flex_1().text_size(body_size)))
            .child(tool_pick(store,"format","编码",&["hex","utf-8","ascii","gbk","latin-1"]))
            .child(tool_pick(store,"checksum","算法",&["sum8","xor8","oddParity","evenParity","crc16Modbus","crc16Ccitt","crc32","lrc"]))
            .child(tool_input(store,"prefix","跳过开头字节")).child(tool_input(store,"suffix","跳过结尾字节"))
            .child(tool_toggle(store,"hex_ending","HEX 发送时允许附加行尾"))
            .child(view().py(px(10.)).items_start().child(run(store,"checksum","计算",feedback))),
    }
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
fn profile(store: Store) -> View {
    let initial = store.read(|s| {
        let chip = s.preferences.get("chip");
        if chip.is_empty() {
            "烧录配置".to_owned()
        } else {
            chip.to_owned()
        }
    });
    let selected = initial.encode_utf16().count() as u32;
    let name = signal(initial);
    let selection = signal((0, 0));
    let first_focus = signal(true);
    let save = move || {
        if name.with_untracked(|name| name.trim().is_empty()) {
            return;
        }
        let result = store.change(|s| {
            let old_name = s.preferences.get("profile").to_owned();
            let profiles = s.preferences.profiles.clone();
            let requested_name = name.get_untracked();
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
                store.change(|s| s.report(format!("已保存配置“{stored_name}”")));
                close(store);
            }
            Err(error) => store.queue_storage_error(error),
        }
    };
    view()
        .w(px(400.))
        .p_6()
        .gap(px(18.))
        .text_size(body_size)
        .line_height(nsh_core::LineHeight::Multiple(1.2))
        .child(
            view()
                .gap(px(3.))
                .child(text("保存烧录配置").font_weight(FontWeight::BOLD))
                .child(caption("保存探针、目标芯片、连接参数与烧录选项。")),
        )
        .child(
            draft_input(
                "配置名称",
                move || name.get(),
                move |value| name.set(value.into()),
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
            .on_submit(move |_: &str| save())
            .focus(|s| {
                s.outline(px(3.5))
                    .outline_color(move || theme().primary.opacity(0.5))
            }),
        )
        .child(
            view()
                .p_3()
                .rounded(px(8.))
                .bg(move || theme().foreground.opacity(0.05))
                .gap(px(6.))
                .children([("目标", 0), ("接口", 1), ("擦除", 2)].into_iter().map(
                    move |(label, index)| {
                        view()
                            .flex_row()
                            .gap(px(14.))
                            .child(
                                text(label)
                                    .w(px(24.5))
                                    .text_size(caption_size)
                                    .text_color(move || theme().muted_foreground),
                            )
                            .child(
                                text(move || {
                                    store.read(|s| match index {
                                        0 => {
                                            let target = s.preferences.get("chip");
                                            if target.is_empty() {
                                                "自动识别".into()
                                            } else {
                                                target.into()
                                            }
                                        }
                                        1 => format!(
                                            "{} · {} kHz",
                                            s.preferences.get("wire").to_uppercase(),
                                            s.preferences.get("speed")
                                        ),
                                        _ => match s.preferences.get("erase") {
                                            "full" => "整片擦除",
                                            "skip" => "跳过擦除",
                                            _ => "按需擦除",
                                        }
                                        .into(),
                                    })
                                })
                                .flex_1()
                                .min_w_0()
                                .text_size(caption_size),
                            )
                    },
                )),
        )
        .child(
            view()
                .flex_row()
                .justify_end()
                .gap_2()
                .child(action(
                    "取消",
                    Variant::Default,
                    || false,
                    move || close(store),
                ))
                .child(action(
                    "保存",
                    Variant::DefaultAction,
                    move || name.with(|name| name.trim().is_empty()),
                    save,
                )),
        )
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

fn tool_toggle(store: Store, key: &'static str, label: &'static str) -> View {
    tool_row(label)
        .min_h(px(37.5))
        .child(view().flex_1())
        .child(
            nsh_base::switch()
                .checked(move || store.flag(key))
                .on_change(move |v: &bool| store.change(|s| s.flag(key, *v)))
                .accessibility_label(label)
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

fn tool_note(label: &'static str) -> Text {
    text(label)
        .text_size(small_size)
        .h(px(12.))
        .line_height(px(12.))
        .text_color(move || theme().muted_foreground)
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
