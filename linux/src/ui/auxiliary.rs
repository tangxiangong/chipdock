use super::*;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Panel {
    Memory,
    Logs,
    Plot(u64),
}
struct Auxiliary {
    owner: Entity<Workbench>,
    panel: Panel,
    config: Entity<TextareaState>,
    sample: Entity<InputState>,
    filter: Entity<InputState>,
    message: String,
    busy: bool,
}
impl Workbench {
    pub(super) fn open_panel(&mut self, panel: Panel, cx: &mut Context<Self>) -> Result<()> {
        if panel == Panel::Logs {
            self.unseen_errors = 0;
            cx.notify();
        }
        if let Some((_, handle)) = self.auxiliary.iter().find(|(kind, _)| *kind == panel)
            && (handle.update(cx, |_, w, _| w.activate_window()).is_ok()
                || cx
                    .windows()
                    .iter()
                    .any(|window| window.window_id() == handle.window_id()))
        {
            return Ok(());
        }
        let owner = cx.entity();
        let config = match panel {
            Panel::Plot(id) if Some(id) == self.active => self.snapshot(cx).get("plot").to_owned(),
            Panel::Plot(id) => self
                .sessions
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.preferences.get("plot").to_owned())
                .context("会话已关闭")?,
            _ => "{}".into(),
        };
        let title = match panel {
            Panel::Memory => "ChipDock · 内存".into(),
            Panel::Logs => "ChipDock · 日志".into(),
            Panel::Plot(id) => format!("ChipDock · 串口 {id} 曲线"),
        };
        let (handle, _) = gpui_kit::open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(800.), px(600.)),
                    cx,
                ))),
                window_min_size: Some(size(px(640.), px(420.))),
                ..Default::default()
            },
            cx,
            |w, cx| {
                cx.new(|cx| {
                    cx.observe(&owner, |_, _, cx| cx.notify()).detach();
                    let filter = cx.new(|cx| InputState::new(w, cx));
                    cx.observe(&filter, |_, _, cx| cx.notify()).detach();
                    Auxiliary {
                        owner,
                        panel,
                        config: cx.new(|cx| TextareaState::new(w, cx).default_value(config)),
                        sample: cx.new(|cx| InputState::new(w, cx)),
                        filter,
                        message: String::new(),
                        busy: false,
                    }
                })
            },
        )?;
        self.auxiliary.retain(|(kind, _)| *kind != panel);
        self.auxiliary.push((panel, handle));
        Ok(())
    }
    pub(super) fn close_panels(&mut self, cx: &mut Context<Self>) {
        for (_, handle) in std::mem::take(&mut self.auxiliary) {
            let _ = handle.update(cx, |_, w, _| w.remove_window());
        }
    }
}
impl Auxiliary {
    fn configure(&mut self, preview: bool, w: &mut Window, cx: &mut Context<Self>) {
        let Panel::Plot(id) = self.panel else { return };
        let source = self.config.read(cx).value().to_string();
        let config = match serde_json::from_str::<Value>(&source) {
            Ok(v) => v,
            Err(e) => {
                self.message = e.to_string();
                cx.notify();
                return;
            }
        };
        let sample = if preview {
            self.sample.read(cx).value().to_string()
        } else {
            String::new()
        };
        let hex = config["mode"] == "binary";
        let task = cx.background_executor().spawn(async move {
            service::serial_workbench(
                id,
                &json!({"action":"plot","config":config,"sample":sample,"hex":hex}).to_string(),
            )
        });
        self.busy = true;
        cx.spawn_in(w, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |s, w, cx| {
                s.busy = false;
                match result {
                    Err(e) => s.message = e.to_string(),
                    Ok(value) => {
                        s.message = value;
                        if !preview {
                            s.owner.update(cx, |owner, cx| {
                                if Some(id) == owner.active {
                                    owner.editors["plot"]
                                        .update(cx, |e, cx| e.set_value(source.clone(), w, cx));
                                    owner.plot_config = source.clone();
                                    owner.plot_revision += 1;
                                    owner.plot_points.clear();
                                    owner.plot_dropped = 0;
                                } else if let Some(session) =
                                    owner.sessions.iter_mut().find(|s| s.id == id)
                                {
                                    session
                                        .preferences
                                        .fields
                                        .insert("plot".into(), source.clone());
                                    session.plot_config = source;
                                    session.plot_revision += 1;
                                    session.plot_points.clear();
                                    session.plot_dropped = 0;
                                }
                                cx.notify();
                            });
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn plot(&mut self, id: u64, cx: &mut Context<Self>) -> AnyElement {
        let state = self.owner.read(cx);
        let Some(session) = state.sessions.iter().find(|s| s.id == id) else {
            return div().child("串口会话已关闭").into_any_element();
        };
        let (points, paused, dropped) = if Some(id) == state.active {
            (
                state.plot_points.clone(),
                !state.plotting,
                state.plot_dropped,
            )
        } else {
            (
                session.plot_points.clone(),
                !session.plotting,
                session.plot_dropped,
            )
        };
        let field = self.filter.read(cx).value().to_string();
        let bytes = chipdock_gpui::presentation::plot_csv(&points, &field);
        let mut page = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Textarea::new(&self.config).h(px(100.)))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("apply")
                            .label("应用配置")
                            .disabled(self.busy)
                            .on_click(cx.listener(|s, _, w, cx| s.configure(false, w, cx))),
                    )
                    .child(
                        Button::new("pause")
                            .label(if paused { "继续" } else { "暂停" })
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.owner.update(cx, |o, cx| {
                                    if Some(id) == o.active {
                                        o.plotting = !o.plotting;
                                    } else if let Some(session) =
                                        o.sessions.iter_mut().find(|s| s.id == id)
                                    {
                                        session.plotting = !session.plotting;
                                    }
                                    cx.notify();
                                })
                            })),
                    )
                    .child(Button::new("clear").label("清空").on_click(cx.listener(
                        move |s, _, _, cx| {
                            s.owner.update(cx, |o, cx| {
                                if Some(id) == o.active {
                                    o.plot_revision += 1;
                                    o.plot_points.clear();
                                    o.plot_dropped = 0;
                                } else if let Some(session) =
                                    o.sessions.iter_mut().find(|s| s.id == id)
                                {
                                    session.plot_revision += 1;
                                    session.plot_points.clear();
                                    session.plot_dropped = 0;
                                }
                                cx.notify();
                            })
                        },
                    )))
                    .child(
                        Button::new("export")
                            .label("导出 CSV…")
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.owner.update(cx, |o, cx| {
                                    o.export_bytes("ChipDock-Plot.csv", bytes.clone(), cx)
                                })
                            })),
                    ),
            )
            .child("试算样例")
            .child(Input::new(&self.sample))
            .child(
                Button::new("preview")
                    .label("试算")
                    .disabled(self.busy)
                    .on_click(cx.listener(|s, _, w, cx| s.configure(true, w, cx))),
            )
            .child("显示字段（留空全部）")
            .child(Input::new(&self.filter))
            .child(format!("丢失 {dropped} 条 · {}", self.message));
        let mut series: BTreeMap<String, Vec<(String, f64)>> = BTreeMap::new();
        for point in points {
            if let Some(values) = point["values"].as_object() {
                for (name, value) in values {
                    if (field.is_empty() || field == *name)
                        && let Some(value) = value.as_f64()
                    {
                        series
                            .entry(name.clone())
                            .or_default()
                            .push((point["seq"].to_string(), value));
                    }
                }
            }
        }
        for (index, (name, data)) in series.into_iter().enumerate() {
            page = page.child(
                div().h(px(220.)).child(name).child(
                    gpui_kit::component::chart::LineChart::new(data)
                        .id(("series", index))
                        .x(|v: &(String, f64)| v.0.clone())
                        .y(|v: &(String, f64)| v.1),
                ),
            );
        }
        page.into_any_element()
    }
}
impl Render for Auxiliary {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.panel {
            Panel::Plot(id) => self.plot(id, cx),
            panel => self.owner.update(cx, |o, cx| {
                let previous = o.page;
                o.page = if panel == Panel::Memory {
                    "memory"
                } else {
                    "logs"
                };
                let view = if panel == Panel::Memory {
                    o.memory_page(cx)
                } else {
                    o.logs_page(cx)
                };
                o.page = previous;
                view
            }),
        };
        let _ = w;
        div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|s, _: &Quit, w, cx| {
                s.owner.update(cx, |o, cx| {
                    if o.busy_lanes.iter().all(|l| l.background())
                        && o.sessions.iter().all(|s| {
                            s.status
                                .as_ref()
                                .is_none_or(|s| s.job_kind.is_none() && !s.logging && !s.capturing)
                        })
                    {
                        o.save(w, cx);
                        cx.quit();
                    } else {
                        o.report_error("请先停止任务和采集", cx);
                    }
                })
            }))
            .child(
                div()
                    .id("panel-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .p_4()
                    .child(body),
            )
    }
}
