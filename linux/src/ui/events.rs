use super::*;
use chipdock_gpui::update;

impl Workbench {
    pub(super) fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.busy_lanes.contains(&Lane::Probe) {
            match chipdock_gpui::instance::take_firmware() {
                Ok(Some(path)) => {
                    self.set("firmware", path.to_string_lossy().into_owned(), window, cx);
                    self.page = "flash";
                    self.submit(Command::Preview(self.snapshot(cx)), cx);
                }
                Ok(None) => {}
                Err(error) => self.report(format!("打开固件失败：{error:#}"), cx),
            }
        }
        if !self.installing && !self.busy_lanes.contains(&Lane::Probe) {
            let p = self.snapshot(cx);
            let key = (p.get("firmware").to_owned(), p.get("base").to_owned());
            if self.firmware_key.as_ref() != Some(&key) {
                self.firmware_key = Some(key.clone());
                self.preview = None;
                self.firmware_output
                    .update(cx, |s, cx| s.set_value("", window, cx));
                if !key.0.is_empty() {
                    self.submit(Command::Preview(p), cx);
                }
            }
        }
        if self.preferences.flag("auto_update")
            && !self.busy_lanes.contains(&Lane::Update)
            && std::time::Instant::now() >= self.next_update_check
        {
            self.next_update_check = std::time::Instant::now() + Duration::from_secs(24 * 60 * 60);
            self.submit(Command::CheckUpdate, cx);
        }
        while let Ok((done, total)) = self.worker.update_progress.try_recv() {
            self.message = format!(
                "下载更新：{done} / {total} 字节（{:.0}%）",
                done as f64 / total as f64 * 100.
            );
            cx.notify();
        }
        for event in self.worker.core.take_events() {
            self.report(
                format!(
                    "{}{}",
                    event.message,
                    event
                        .progress
                        .map(|p| format!("  {:.0}%", p * 100.))
                        .unwrap_or_default()
                ),
                cx,
            );
            self.message = event.message;
            self.progress = event.progress;
            self.completed = matches!(event.stage, JobStage::Completed);
        }
        while let Ok((lane, result)) = self.worker.receiver.try_recv() {
            self.busy_lanes.retain(|l| *l != lane);
            let job = if lane == Lane::Probe {
                self.progress = None;
                self.operation.take()
            } else {
                None
            };
            let was_installing = self.installing && lane == Lane::Update;
            if lane == Lane::Update {
                self.installing = false;
            }
            if was_installing && result.is_err() {
                self.preferences.flags.insert("auto_install".into(), false);
            }
            match result {
                Err(e) => {
                    if let Lane::Plot(id) = lane {
                        if Some(id) == self.active {
                            self.plotting = false;
                        } else if let Some(session) = self.sessions.iter_mut().find(|s| s.id == id)
                        {
                            session.plotting = false;
                        }
                    }
                    if let Some(chipdock_service::records::CoreError::Operation { response }) =
                        e.downcast_ref::<chipdock_service::records::CoreError>()
                    {
                        self.chips = response
                            .target_candidates
                            .iter()
                            .map(|c| c.name.clone())
                            .collect();
                        let message = format!("{}\n{}", response.message, response.recovery);
                        if let Some(detail) = &response.detail {
                            self.logs.push_back(format!("[错误] {detail}"));
                        }
                        self.report_error(message, cx);
                        if job.is_some() {
                            let detail = [
                                response.detail.clone().unwrap_or_default(),
                                response.recovery.clone(),
                            ]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join("\n\n");
                            self.present_failure(response.message.clone(), detail, window, cx);
                        }
                    } else {
                        self.report_error(format!("{e:#}"), cx);
                        if job.is_some() {
                            self.present_failure("操作失败".into(), format!("{e:#}"), window, cx);
                        }
                    }
                }
                Ok(out) => {
                    if let Some(message) = completion_status(
                        lane,
                        &out,
                        self.busy_lanes.iter().all(|l| l.background()),
                    ) {
                        self.message = message.into();
                    }
                    match out {
                        Output::Update(update) => {
                            self.downloaded = None;
                            self.update = update;
                            self.report(
                                if self.update.is_some() {
                                    "发现新的 GPUI 版本"
                                } else {
                                    "当前已是最新版本"
                                },
                                cx,
                            );
                            if self.preferences.flag("auto_download")
                                && update::can_self_update()
                                && let Some(update) = self.update.clone()
                            {
                                self.submit(Command::DownloadUpdate(update), cx);
                            }
                        }
                        Output::Downloaded(path) => {
                            self.downloaded = Some(path);
                            self.report("更新已下载并校验，安装前请断开串口", cx);
                            if self.preferences.flag("auto_install")
                                && update::can_self_update()
                                && self.sessions.iter().all(|s| {
                                    s.status.as_ref().is_none_or(|s| {
                                        !s.connected
                                            && !s.reconnecting
                                            && !s.logging
                                            && !s.capturing
                                    })
                                })
                                && let (Some(path), Some(update)) =
                                    (self.downloaded.clone(), self.update.clone())
                            {
                                self.request_install(path, update, cx);
                            }
                        }
                        Output::Restart => {
                            cx.quit();
                        }
                        Output::CommandSent(id, text, value) => {
                            if Some(id) == self.active {
                                self.preferences.command_history.retain(|s| s != &text);
                                self.preferences.command_history.insert(0, text);
                                self.preferences.command_history.truncate(50);
                                self.show_text(value.to_string(), window, cx);
                            } else if let Some(session) =
                                self.sessions.iter_mut().find(|s| s.id == id)
                            {
                                session.preferences.command_history.retain(|s| s != &text);
                                session.preferences.command_history.insert(0, text);
                                session.preferences.command_history.truncate(50);
                                session.result = value.to_string();
                            }
                        }
                        Output::Ipc(mode) => {
                            self.preferences.ipc = mode;
                            self.save(window, cx);
                        }
                        Output::Sent(id, text) => {
                            if Some(id) != self.active {
                                if let Some(session) = self.sessions.iter_mut().find(|s| s.id == id)
                                {
                                    session.preferences.history.retain(|s| s != &text);
                                    session.preferences.history.insert(0, text);
                                    session.preferences.history.truncate(50);
                                }
                                continue;
                            }
                            self.preferences.history.retain(|s| s != &text);
                            self.preferences.history.insert(0, text);
                            self.preferences.history.truncate(50);
                            self.report("已发送", cx);
                        }
                        Output::Imported(key, text) => {
                            if let Some(editor) = self.editors.get(&key) {
                                editor.update(cx, |s, cx| s.set_value(text, window, cx));
                            }
                        }
                        Output::Text(s) => {
                            self.show_text(s.clone(), window, cx);
                            self.report(s, cx);
                        }
                        Output::Probes(v) => {
                            self.probes = v;
                            self.has_scanned = true;
                        }
                        Output::Chips(v) => self.chips = v,
                        Output::Ports(v) => self.ports = v,
                        Output::Connected(info) => {
                            self.target_info = info.target_information.clone();
                            self.set("chip", info.chip.clone(), window, cx);
                            self.set("probe", info.probe, window, cx);
                            self.set(
                                "wire",
                                match info.protocol {
                                    WireProtocol::Swd => "swd",
                                    WireProtocol::Jtag => "jtag",
                                },
                                window,
                                cx,
                            );
                            if let Some(speed) = info.speed_khz {
                                self.set("speed", speed.to_string(), window, cx);
                            }
                            self.preferences
                                .flags
                                .insert("under_reset".into(), info.connect_under_reset);
                            self.connected_target = Some(self.snapshot(cx).target_key());
                        }
                        Output::Preview(p, input) => {
                            let current = self.snapshot(cx).firmware();
                            if !current.as_ref().is_ok_and(|v| {
                                v.path == input.path && v.base_address == input.base_address
                            }) {
                                continue;
                            }
                            let path = input.path;
                            self.preferences.recent_bases.insert(
                                path.clone(),
                                input
                                    .base_address
                                    .map(|n| format!("0x{n:X}"))
                                    .unwrap_or_default(),
                            );
                            self.firmware_output.update(cx, |s, cx| {
                                s.set_value(model::hex_dump(p.start_address, &p.bytes), window, cx)
                            });
                            self.preview = Some(p);
                            self.preferences.recent_used.insert(
                                path.clone(),
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map_or(0, |d| d.as_secs()),
                            );
                            self.preferences.recent.retain(|p| p != &path);
                            self.preferences.recent.insert(0, path);
                            self.preferences.recent.truncate(12);
                            let recent = &self.preferences.recent;
                            self.preferences
                                .recent_bases
                                .retain(|k, _| recent.contains(k));
                            self.preferences
                                .recent_used
                                .retain(|k, _| recent.contains(k));
                        }
                        Output::Memory(m) => {
                            match memory_text(&m) {
                                Ok(s) => self
                                    .memory_output
                                    .update(cx, |state, cx| state.set_value(s, window, cx)),
                                Err(e) => self.report_error(e.to_string(), cx),
                            }
                            self.memory = Some(m);
                        }
                        Output::Layout(v) => self.layout = v,
                        Output::Session(id, warning) => {
                            self.sessions.push(Session {
                                id,
                                cursor: 0,
                                cleared_through: 0,
                                frames: VecDeque::new(),
                                status: None,
                                preferences: self.snapshot(cx),
                                plot_points: vec![],
                                plot_cursor: 0,
                                plot_revision: 0,
                                plot_dropped: 0,
                                plot_config: String::new(),
                                plotting: false,
                                paused: false,
                                result: String::new(),
                            });
                            self.switch_session(id, window, cx);
                            if let Some(warning) = warning {
                                self.report_error(warning, cx);
                            }
                            if self.sessions.len() == 1
                                && self.preferences.flag("auto_connect")
                                && !self.snapshot(cx).get("port").is_empty()
                            {
                                self.submit(Command::ConnectSerial(id, self.snapshot(cx)), cx);
                            }
                        }
                        Output::Closed(id) => {
                            self.sessions.retain(|s| s.id != id);
                            if self.active == Some(id) {
                                self.active = None;
                                if let Some(next) = self.sessions.first().map(|s| s.id) {
                                    self.switch_session(next, window, cx);
                                } else {
                                    self.render_terminal(window, cx);
                                }
                            }
                        }
                        Output::Serial(id, status) => {
                            if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                                s.status = Some(status);
                            }
                        }
                        Output::Plot(id, revision, value) => {
                            if Some(id) == self.active && revision == self.plot_revision {
                                chipdock_gpui::presentation::append_plot(
                                    &mut self.plot_points,
                                    &mut self.plot_cursor,
                                    &mut self.plot_dropped,
                                    &value,
                                );
                            } else if Some(id) != self.active
                                && let Some(session) = self.sessions.iter_mut().find(|s| s.id == id)
                                && revision == session.plot_revision
                            {
                                chipdock_gpui::presentation::append_plot(
                                    &mut session.plot_points,
                                    &mut session.plot_cursor,
                                    &mut session.plot_dropped,
                                    &value,
                                );
                            }
                        }
                        Output::Workbench(id, value) => {
                            if let Some(session) = self.sessions.iter_mut().find(|s| s.id == id) {
                                session.result =
                                    serde_json::to_string_pretty(&value).unwrap_or_default();
                            }
                            if Some(id) == self.active {
                                self.show_text(
                                    serde_json::to_string_pretty(&value).unwrap_or_default(),
                                    window,
                                    cx,
                                );
                            }
                        }
                    }
                }
            }
            cx.notify();
        }
        if self.preferences.flag("auto_install")
            && update::can_self_update()
            && self.busy_lanes.iter().all(|l| l.background())
            && self.sessions.iter().all(|s| {
                s.status.as_ref().is_some_and(|v| {
                    !v.connected
                        && !v.reconnecting
                        && !v.logging
                        && !v.capturing
                        && v.job_kind.is_none()
                })
            })
            && let (Some(path), Some(update)) = (self.downloaded.clone(), self.update.clone())
        {
            self.request_install(path, update, cx);
        }
        self.poll_serial(window, cx);
    }
    pub(super) fn show_text(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.output
            .update(cx, |s, cx| s.set_value(text, window, cx));
    }
}

fn completion_status(lane: Lane, out: &Output, idle: bool) -> Option<&'static str> {
    if lane.background() || !idle {
        return None;
    }
    Some(
        if matches!(
            out,
            Output::Probes(_)
                | Output::Ports(_)
                | Output::Session(..)
                | Output::Chips(_)
                | Output::Preview(..)
        ) {
            "准备就绪"
        } else {
            "已完成"
        },
    )
}
#[cfg(test)]
mod status_tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn discovery_and_overlapping_work_do_not_replace_foreground_status() {
        let probes = Output::Probes(Vec::new());
        assert_eq!(
            completion_status(Lane::Probe, &probes, true),
            Some("准备就绪")
        );
        assert_eq!(completion_status(Lane::Probe, &probes, false), None);
        assert_eq!(
            completion_status(Lane::Discovery, &Output::Ports(Vec::new()), true),
            None
        );
    }
}
