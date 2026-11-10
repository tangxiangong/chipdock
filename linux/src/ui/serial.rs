use super::*;

impl Workbench {
    pub(super) fn serial_page(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(self.terminal_page(window, cx));
        if let Some(status) = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref())
        {
            if let Some(error) = status
                .last_error
                .as_ref()
                .or(status.job_error.as_ref())
                .or(status.capture_error.as_ref())
            {
                content = div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .text_color(cx.theme().danger)
                            .child(error.clone()),
                    )
                    .child(content);
            }
            if let Some(feedback) = &status.feedback {
                content = content.child(div().px_3().py_1().text_sm().child(feedback.clone()));
            }
        }
        let mut workspace = h_resizable("serial-workspace");
        if !self.expanded.contains("hide-connection") {
            workspace = workspace.child(
                resizable_panel()
                    .size(px(320.))
                    .size_range(px(220.)..px(340.))
                    .child(self.connection_panel(window, cx)),
            );
        }
        workspace = workspace.child(
            resizable_panel()
                .size_range(px(440.)..px(2400.))
                .child(div().size_full().flex().flex_col().child(content)),
        );
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(32.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().flex_1().min_w_0().flex().gap_1().children(
                        self.sessions.iter().map(|session| {
                            let id = session.id;
                            native_button(("session", id as usize))
                                .small()
                                .ghost()
                                .label(format!("会话 {id}"))
                                .selected(Some(id) == self.active)
                                .on_click(
                                    cx.listener(move |s, _, w, cx| s.switch_session(id, w, cx)),
                                )
                        }),
                    ))
                    .when(self.sessions.len() > 1, |v| {
                        v.child(
                            self.button("close-session", "关闭会话", cx, |s, _, cx| {
                                if let Some(id) = s.active {
                                    s.submit(Command::CloseSession(id), cx)
                                }
                            })
                            .ghost()
                            .icon(icons::icon("close")),
                        )
                    })
                    .child(
                        native_button("local-control")
                            .small()
                            .ghost()
                            .icon(icons::icon("terminal"))
                            .label("本地控制")
                            .child(icons::icon("down"))
                            .dropdown_menu({
                                let owner = cx.entity().downgrade();
                                let current = self.preferences.ipc;
                                move |mut menu, _, _| {
                                    for (mode, label) in [(0, "关闭"), (1, "只读"), (2, "允许控制")]
                                    {
                                        let owner = owner.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(label)
                                                .checked(current == mode)
                                                .on_click(move |_, _, cx| {
                                                    let _ = owner.update(cx, |s, cx| {
                                                        s.submit(Command::Ipc(mode), cx)
                                                    });
                                                }),
                                        );
                                    }
                                    menu
                                }
                            }),
                    )
                    .child(
                        native_button("new-session")
                            .small()
                            .ghost()
                            .icon(icons::icon("plus"))
                            .tooltip("新建串口会话")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.submit(Command::NewSession(s.snapshot(cx)), cx)
                            })),
                    ),
            )
            .child(div().flex_1().min_h_0().child(workspace))
            .child(self.serial_status_bar(cx))
            .into_any_element()
    }

    pub(super) fn open_serial_tool(
        &mut self,
        key: &'static str,
        title: &'static str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if key == "protocol" {
            self.open_protocol(w, cx);
            return;
        }
        self.serial_page = if key == "tools" { "checksum" } else { key };
        self.show_text(String::new(), w, cx);
        let owner = cx.entity().downgrade();
        let closing = owner.clone();
        if !w.has_active_dialog(cx) {
            self.focus_handle.focus(w, cx);
        }
        w.open_dialog(cx, move |dialog, w, cx| {
            let body = owner
                .update(cx, |s, cx| s.serial_tool_body(w, cx))
                .unwrap_or_else(|_| div().into_any_element());
            let closing = closing.clone();
            let tools = matches!(key, "tools" | "checksum" | "framing" | "modbus" | "capture");
            let workflow = matches!(key, "packets" | "file" | "macro" | "replies");
            let records = key == "records";
            dialog
                .when(!tools && !workflow && !records, |d| d.title(title))
                .p_0()
                .close_button(!tools && !workflow && !records)
                .w(px(if tools {
                    540.
                } else if workflow {
                    660.
                } else if records {
                    920.
                } else {
                    760.
                }))
                .child(
                    div()
                        .id("serial-dialog-body")
                        .text_size(px(13.))
                        .line_height(relative(1.2))
                        .max_h(px(640.))
                        .overflow_y_scroll()
                        .child(body),
                )
                .on_close(move |_, _, cx| {
                    let _ = closing.update(cx, |s, cx| {
                        s.serial_page = "terminal";
                        cx.notify();
                    });
                })
        });
    }
    fn serial_tool_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if matches!(
            self.serial_page,
            "checksum" | "framing" | "modbus" | "capture"
        ) {
            return self.tools_page(cx);
        }
        if matches!(self.serial_page, "packets" | "file" | "macro" | "replies") {
            return self.workflow_page(cx);
        }
        if self.serial_page == "records" {
            return self.records_page(cx).into_any_element();
        }
        let body = match self.serial_page {
            "terminal" => div().size_full().child(self.terminal_page(window,cx)),
            "records" => self.records_page(cx),
            "packets" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.editor("packets", cx))
                .child(self.row(&["selected", "interval"]))
                .child(self.button("validate-packets", "验证包库", cx, |s, _, cx| {
                    s.serial_action("validatePackets", cx)
                }))
                .child(self.button("batch", "批量发送", cx, |s, _, cx| {
                    s.serial_action("batch", cx)
                })),
            "file" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.row(&["file", "chunk", "interval"]))
                .child(self.button("pick-file", "选择文件…", cx, |s, _, cx| {
                    s.pick("file", cx)
                }))
                .child(self.button("send-file", "发送文件", cx, |s, _, cx| {
                    s.serial_action("file", cx)
                })),
            "macro" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.editor("macro", cx))
                .child(self.field("cycles"))
                .child(self.button("run-macro", "执行宏", cx, |s, _, cx| {
                    s.serial_action("macro", cx)
                }))
                .child(self.field("export"))
                .child(self.button("macro-report", "导出宏报告", cx, |s, _, cx| {
                    s.serial_action("exportMacro", cx)
                })),
            "replies" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.editor("replies", cx))
                .child(
                    self.button("apply-replies", "应用自动应答", cx, |s, _, cx| {
                        s.serial_action("replies", cx)
                    }),
                ),
            "protocol" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.editor("protocol", cx))
                .child(
                    self.button("apply-protocol", "应用协议模板", cx, |s, _, cx| {
                        s.serial_action("protocol", cx)
                    }),
                )
                .child(self.field("protocol_sample"))
                .child(self.button("decode-sample","测试接收解析",cx,|s,_,cx|{let p=s.snapshot(cx);match serde_json::from_str::<Value>(p.get("protocol")){Ok(rules)=>s.submit(Command::Protocol(json!({"operation":"decode","rules":rules,"incoming":p.get("protocol_sample")})),cx),Err(e)=>s.report(e.to_string(),cx)}}))
                 .child(self.field("protocol_rule"))
                .child(self.button("inspect-protocol","查看协议帧布局",cx,|s,_,cx|{let p=s.snapshot(cx);match serde_json::from_str::<Value>(p.get("protocol")){Ok(rules)=>s.submit(Command::Protocol(json!({"operation":"inspect","rules":rules,"input":p.get("command"),"incoming":p.get("protocol_sample"),"rule":p.get("protocol_rule")})),cx),Err(e)=>s.report_error(e.to_string(),cx)}}))
                .child(self.field("command"))
                .child(div().flex().flex_wrap().gap_1().children(self.preferences.command_history.iter().take(12).enumerate().map(|(index,text)|{let text=text.clone();native_button(("command-history",index)).label(text.clone()).on_click(cx.listener(move |s,_,w,cx|s.set("command",text.clone(),w,cx)))})))
                .child(self.button("preview-command","预览命令编码",cx,|s,_,cx|{let p=s.snapshot(cx);match serde_json::from_str::<Value>(p.get("protocol")){Ok(rules)=>s.submit(Command::Protocol(json!({"operation":"encode","rules":rules,"input":p.get("command")})),cx),Err(e)=>s.report(e.to_string(),cx)}}))
                .child(self.button("command", "发送协议命令", cx, |s, _, cx| {
                    s.serial_action("command", cx)
                })),
            "capture" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.row(&["capture_path", "roll"]))
                .child(self.button("capture-directory","选择采集目录…",cx,|s,_,cx|s.pick_capture_directory(cx)))
                .child(self.choice("capture_format", &["txt", "csv", "bin"], window, cx))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(self.button("capture", "开始日志", cx, |s, _, cx| {
                            s.serial_action("capture", cx)
                        }))
                        .child(self.button("stop-capture", "停止日志", cx, |s, _, cx| {
                            s.serial_action("stopCapture", cx)
                        }))
                        .child(self.button("raw", "开始原始采集", cx, |s, _, cx| {
                            s.serial_action("raw", cx)
                        }))
                        .child(self.button("stop-raw", "停止原始采集", cx, |s, _, cx| {
                            s.serial_action("stopRaw", cx)
                        })),
                )
                .child(self.field("export"))
                .child(self.button("serial-export", "导出记录", cx, |s, _, cx| {
                    s.serial_action("export", cx)
                })),
            "plot" => self.plot_page(cx),
            _ => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.button("calculate", "计算发送校验", cx, |s, _, cx| {
                    s.serial_action("checksum", cx)
                }))
                .child(self.choice(
                    "frame_mode",
                    &["read", "idle", "header", "tail", "fixed"],
                    window,
                    cx,
                ))
                .child(self.field("frame_value"))
                .child(self.button("framing", "应用分帧", cx, |s, _, cx| {
                    s.serial_action("framing", cx)
                }))
                .child(div().text_lg().child("Modbus RTU"))
                .child(self.row(&["station", "register", "quantity", "timeout"]))
                .child(self.choice(
                    "function",
                    &["1", "2", "3", "4", "5", "6", "15", "16"],
                    window,
                    cx,
                ))
                .child(self.field("values"))
                .child(self.button("modbus", "生成请求", cx, |s, _, cx| {
                    s.serial_action("modbus", cx)
                }))
                .child(
                    self.button("modbus-send", "发送并接收响应", cx, |s, _, cx| {
                        s.serial_action("modbusSend", cx)
                    }),
                ),
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(body)
            .child(self.button("cancel-job", "停止发送任务", cx, |s, _, cx| {
                s.serial_action("cancelJob", cx)
            }))
            .child(Textarea::new(&self.output).h(px(120.)).readonly(true))
            .into_any_element()
    }
}
