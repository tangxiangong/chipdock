use super::*;

#[derive(Clone, Copy)]
pub(super) struct Feedback {
    pub session: Option<u64>,
    pub result: Signal<String>,
    pub error: Signal<String>,
    pub running: Signal<bool>,
    pub label_height: Signal<f32>,
    revision: Signal<u64>,
    pending: Signal<Option<String>>,
    key: Signal<&'static str>,
    generation: Signal<u64>,
}

impl Feedback {
    pub fn new(store: Store) -> Self {
        let feedback = Self {
            session: store
                .tool_session
                .get_untracked()
                .or(store.read(|s| s.active)),
            result: signal(String::new()),
            error: signal(String::new()),
            running: signal(false),
            label_height: signal(16.),
            revision: signal(0),
            pending: signal(None),
            key: signal(""),
            generation: signal(0),
        };
        feedback.reset(store);
        effect(move || {
            let Some(action) = feedback.pending.get() else {
                return;
            };
            let Some(operation) = store.read(|s| {
                feedback
                    .session
                    .and_then(|id| s.sessions.get(&id))
                    .and_then(|session| session.operation_result.clone())
            }) else {
                return;
            };
            if operation.revision <= feedback.revision.get() || operation.action != action {
                return;
            }
            feedback.revision.set(operation.revision);
            let key = feedback.key.get_untracked();
            feedback.finish(operation.outcome.map(|value| result_text(key, &value)));
        });
        feedback
    }

    pub fn reset(self, store: Store) {
        self.generation.update(|value| *value += 1);
        self.result.set(String::new());
        self.error.set(String::new());
        self.pending.set(None);
        self.running.set(false);
        self.label_height.set(16.);
        self.revision.set(store.read(|s| {
            self.session
                .and_then(|id| s.sessions.get(&id))
                .and_then(|session| session.operation_result.as_ref())
                .map_or(0, |operation| operation.revision)
        }));
    }

    pub fn begin(self, store: Store) -> u64 {
        self.reset(store);
        self.running.set(true);
        self.generation.get_untracked()
    }

    pub fn current(self, generation: u64) -> bool {
        self.generation.try_get() == Some(generation)
    }

    pub fn finish(self, outcome: Result<String, String>) {
        if self.running.try_get().is_none() {
            return;
        }
        self.running.set(false);
        self.pending.set(None);
        match outcome {
            Ok(text) => {
                self.result.set(text);
                self.error.set(String::new());
            }
            Err(error) => {
                self.result.set(String::new());
                self.error.set(error);
            }
        }
    }

    pub fn dispatch(self, store: Store, key: &'static str) {
        let request = store.read(|s| {
            let preferences = if s.active == self.session {
                Some(&s.preferences)
            } else {
                self.session
                    .and_then(|id| s.sessions.get(&id))
                    .map(|session| &session.preferences)
            }
            .ok_or_else(|| "串口会话已关闭".to_owned())?;
            crate::serial::request(preferences, key).map_err(|error| error.to_string())
        });
        match request {
            Ok(request) => self.dispatch_request(store, key, request),
            Err(error) => {
                self.reset(store);
                self.finish(Err(error));
            }
        }
    }

    pub fn dispatch_request(self, store: Store, key: &'static str, request: Value) {
        let Some(id) = self.session else {
            self.finish(Err("串口会话已关闭".into()));
            return;
        };
        if store.read(|s| s.installing || s.busy.contains(&Lane::Session(id))) {
            self.finish(Err("请等待当前操作完成".into()));
            return;
        }
        let Some(action) = request.get("action").and_then(Value::as_str) else {
            self.finish(Err("操作缺少类型".into()));
            return;
        };
        self.begin(store);
        self.key.set(key);
        self.pending.set(Some(action.to_owned()));
        store.command(move |_| Command::Workbench(id, request));
    }

    pub fn dispatch_framing(self, store: Store, mode: String, value: String) {
        if matches!(mode.as_str(), "idle" | "fixed")
            && !value.parse::<i64>().is_ok_and(|number| number > 0)
        {
            self.reset(store);
            self.finish(Err("请输入正整数".into()));
            return;
        }
        let Some(id) = self.session else {
            self.finish(Err("串口会话已关闭".into()));
            return;
        };
        let request = store.read(|s| {
            let mut preferences = if s.active == Some(id) {
                Some(s.preferences.clone())
            } else {
                s.sessions
                    .get(&id)
                    .map(|session| session.preferences.clone())
            }
            .ok_or_else(|| "串口会话已关闭".to_owned())?;
            preferences.fields.insert("frame_mode".into(), mode.clone());
            preferences
                .fields
                .insert("frame_value".into(), value.clone());
            crate::serial::request(&preferences, "framing").map_err(|error| error.to_string())
        });
        let request = match request {
            Ok(request) => request,
            Err(error) => {
                self.reset(store);
                self.finish(Err(error));
                return;
            }
        };
        self.begin(store);
        self.key.set("framing");
        self.pending.set(Some("framing".into()));
        let baseline = self.revision.get_untracked();
        if !store.change(|s| s.submit_framing(id, request, mode, value)) {
            let operation = store.read(|s| {
                s.sessions
                    .get(&id)
                    .and_then(|session| session.operation_result.clone())
                    .filter(|operation| {
                        operation.revision > baseline && operation.action == "framing"
                    })
            });
            if let Some(operation) = operation {
                self.revision.set(operation.revision);
                self.finish(
                    operation
                        .outcome
                        .map(|value| result_text("framing", &value)),
                );
            } else {
                self.finish(Err(if store.read(|s| s.sessions.contains_key(&id)) {
                    "请等待当前操作完成"
                } else {
                    "串口会话已关闭"
                }
                .into()));
            }
        }
    }
}

fn result_text(key: &str, value: &Value) -> String {
    match key {
        "framing" => "已应用分帧设置".into(),
        "file" | "macro" | "batch" => "发送任务已启动".into(),
        "replies" => "已应用应答规则".into(),
        "exportMacro" => "已导出宏结果".into(),
        "capture" | "raw" | "stopCapture" | "stopRaw" => String::new(),
        _ => value.to_string(),
    }
}

pub(super) fn result_box(feedback: Feedback) -> View {
    view()
        .label("结果")
        .gap(px(4.))
        .child(
            text("结果")
                .text_size(small_control_size)
                .font_weight(FontWeight::SEMIBOLD)
                .line_height(px(13.))
                .h(px(13.))
                .pl(px(10.)),
        )
        .child(
            view()
                .p(px(5.))
                .rounded(px(12.))
                .bg(move || theme().color("tool-result-well").unwrap_or(theme().card))
                .child(
                    scroll().h(px(110.)).child(
                        text(move || feedback.result.get())
                            .font_family(mono)
                            .text_size(caption_size)
                            .line_height(px(15.))
                            .selectable(true)
                            .w_full(),
                    ),
                ),
        )
}

pub(super) fn message(feedback: Feedback, failed: bool) -> View {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .on_layout(move |bounds: &Bounds<Points>| {
            let height = bounds.size.height.0;
            if (height - feedback.label_height.get_untracked()).abs() > 0.25 {
                feedback.label_height.set(height);
            }
        })
        .child(
            view()
                .size(px(16.))
                .items_center()
                .justify_center()
                .child(if failed {
                    icons::icon("warning-fill")
                        .size(px(15.))
                        .text_color(move || {
                            theme().color("tool-error").unwrap_or(theme().destructive)
                        })
                        .into_element()
                } else {
                    view()
                        .size(px(13.))
                        .rounded_full()
                        .bg(move || theme().muted_foreground)
                        .items_center()
                        .justify_center()
                        .child(
                            icons::icon("check")
                                .size(px(9.))
                                .text_color(move || theme().background),
                        )
                        .into_element()
                }),
        )
        .child(
            text(move || {
                if failed {
                    feedback.error.get()
                } else {
                    feedback.result.get()
                }
            })
            .text_size(body_size)
            .line_height(px(16.))
            .flex_1()
            .min_w_0()
            .selectable(true)
            .text_color(move || {
                if failed {
                    theme().color("tool-error").unwrap_or(theme().destructive)
                } else {
                    theme().muted_foreground
                }
            }),
        )
}

struct JobSnapshot {
    kind: Option<String>,
    done: u64,
    total: u64,
    error: Option<String>,
}

fn job_state(store: Store, feedback: Feedback) -> Option<JobSnapshot> {
    store.read(|s| {
        feedback
            .session
            .and_then(|id| s.sessions.get(&id))
            .and_then(|session| session.status.as_ref())
            .map(|status| JobSnapshot {
                kind: status.job_kind.clone(),
                done: status.job_done,
                total: status.job_total,
                error: status.job_error.clone(),
            })
    })
}

pub(super) fn job_height(store: Store, feedback: Feedback) -> f32 {
    match job_state(store, feedback) {
        Some(JobSnapshot { kind: Some(_), .. }) => 36.,
        Some(JobSnapshot {
            kind: None, total, ..
        }) if total > 0 => 25.,
        _ => 0.,
    }
}

pub(super) fn job(store: Store, feedback: Feedback) -> impl IntoElement {
    when(
        move || job_height(store, feedback) > 0.,
        move || {
            when(
                move || job_state(store, feedback).is_some_and(|s| s.kind.is_some()),
                move || {
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(when(
                            move || job_state(store, feedback).is_some_and(|s| s.total > 0),
                            move || {
                                nsh_base::progress()
                                    .flex_1()
                                    .min_w_0()
                                    .value(move || {
                                        job_state(store, feedback).map_or(0., |s| {
                                            if s.total > 0 {
                                                s.done as f32 / s.total as f32 * 100.
                                            } else {
                                                0.
                                            }
                                        })
                                    })
                                    .accessibility_label("串口任务进度")
                                    .track_style(|s| {
                                        s.h(px(4.))
                                            .w_full()
                                            .rounded_full()
                                            .bg(move || theme().foreground.opacity(0.1))
                                    })
                                    .indicator_style(|s| {
                                        s.h(px(4.)).rounded_full().bg(move || theme().primary)
                                    })
                            },
                        ))
                        .child(
                            text(move || {
                                job_state(store, feedback).map_or_else(String::new, |s| {
                                    format!(
                                        "{} · {} / {}",
                                        s.kind.unwrap_or_default(),
                                        s.done,
                                        s.total
                                    )
                                })
                            })
                            .text_size(small_size)
                            .tabular_nums(),
                        )
                        .child(action(
                            "取消任务",
                            Variant::Default,
                            move || {
                                store.read(|s| {
                                    s.installing
                                        || feedback
                                            .session
                                            .is_some_and(|id| s.busy.contains(&Lane::Session(id)))
                                })
                            },
                            move || {
                                if let Some(id) = feedback.session {
                                    store.command(move |_| {
                                        Command::Workbench(
                                            id,
                                            serde_json::json!({"action":"cancelJob"}),
                                        )
                                    });
                                }
                            },
                        ))
                },
            )
            .otherwise(move || {
                text(move || {
                    job_state(store, feedback).map_or_else(String::new, |s| {
                        format!(
                            "{} / {} · {}",
                            s.done,
                            s.total,
                            s.error.as_deref().unwrap_or("已完成")
                        )
                    })
                })
                .text_size(small_size)
                .line_height(px(13.))
            })
        },
    )
}
