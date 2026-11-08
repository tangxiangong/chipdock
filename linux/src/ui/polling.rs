use super::*;

impl Workbench {
    pub(super) fn poll_serial(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.installing
            && std::time::Instant::now() >= self.next_port_scan
            && !self.busy_lanes.contains(&Lane::Discovery)
        {
            self.next_port_scan = std::time::Instant::now() + Duration::from_secs(1);
            self.submit(Command::Ports, cx);
        }
        while let Ok((id, result)) = self.poll_rx.try_recv() {
            self.poll_pending.remove(&id);
            if !self.sessions.iter().any(|s| s.id == id) {
                continue;
            }
            match result {
                Ok(batch) => {
                    if batch.dropped_frames > 0 {
                        self.report(format!("接收缓冲区已淘汰 {} 帧", batch.dropped_frames), cx);
                    }
                    let connection = if batch.status.connected
                        && !self.busy_lanes.contains(&Lane::Session(id))
                    {
                        batch.status.path.clone().zip(batch.status.config.clone())
                    } else {
                        None
                    };
                    if let Some((path, config)) = &connection {
                        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == id) {
                            session.preferences.apply_connection(path, config);
                        }
                        if Some(id) == self.active {
                            let mut p = self.snapshot(cx);
                            p.apply_connection(path, config);
                            for key in ["port", "baud", "bits", "stop", "parity", "flow"] {
                                if self.fields[key].read(cx).value().as_ref() != p.get(key) {
                                    self.set(key, p.get(key).to_owned(), window, cx);
                                }
                            }
                        }
                    }
                    if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                        let changed = !batch.frames.is_empty()
                            || s.status.as_ref().is_none_or(|old| {
                                old.connected != batch.status.connected
                                    || old.reconnecting != batch.status.reconnecting
                                    || old.rx_bytes != batch.status.rx_bytes
                                    || old.tx_bytes != batch.status.tx_bytes
                                    || old.rx_per_second != batch.status.rx_per_second
                                    || old.tx_per_second != batch.status.tx_per_second
                                    || old.job_done != batch.status.job_done
                                    || old.job_kind != batch.status.job_kind
                                    || old.last_error != batch.status.last_error
                                    || old.capture_error != batch.status.capture_error
                                    || old.logging != batch.status.logging
                                    || old.capturing != batch.status.capturing
                                    || old.feedback != batch.status.feedback
                            });
                        if changed {
                            cx.notify();
                        }
                        s.cursor = batch.next_cursor;
                        s.status = Some(batch.status);
                        s.frames.extend(batch.frames);
                        while s.frames.len() > 2000
                            || s.frames
                                .iter()
                                .map(|f| {
                                    f.bytes.len()
                                        + f.hex.len()
                                        + f.text.utf8.len()
                                        + f.text.gbk.len()
                                        + f.text.ascii.len()
                                        + f.text.latin1.len()
                                })
                                .sum::<usize>()
                                > 4 * 1024 * 1024
                        {
                            s.frames.pop_front();
                        }
                    }
                    if !self.paused && Some(id) == self.active {
                        self.render_terminal(window, cx);
                    }
                }
                Err(e) => self.report_error(e.to_string(), cx),
            }
        }
        for session in &self.sessions {
            if !self.poll_pending.contains(&session.id)
                && self.poll_tx.send((session.id, session.cursor)).is_ok()
            {
                self.poll_pending.insert(session.id);
            }
        }
        if !self.installing {
            let mut plots = Vec::new();
            if self.plotting
                && let Some(id) = self.active
                && !self.busy_lanes.contains(&Lane::Plot(id))
            {
                let source = self.snapshot(cx).get("plot").to_owned();
                if source != self.plot_config {
                    self.plot_config = source.clone();
                    self.plot_revision += 1;
                    self.plot_points.clear();
                    self.plot_dropped = 0;
                }
                plots.push((id, self.plot_revision, self.plot_cursor, source));
            }
            for session in &self.sessions {
                if Some(session.id) != self.active
                    && session.plotting
                    && !self.busy_lanes.contains(&Lane::Plot(session.id))
                {
                    plots.push((
                        session.id,
                        session.plot_revision,
                        session.plot_cursor,
                        session.preferences.get("plot").to_owned(),
                    ));
                }
            }
            for (id, revision, cursor, source) in plots {
                match serde_json::from_str::<Value>(&source) {
                    Ok(config) => self.submit(
                        Command::Plot(
                            id,
                            revision,
                            json!({"action":"plot","config":config,"cursor":cursor}),
                        ),
                        cx,
                    ),
                    Err(e) => {
                        if Some(id) == self.active {
                            self.plotting = false;
                        }
                        self.report_error(e.to_string(), cx);
                    }
                }
            }
        }
        if let Some((request, id)) = service::ipc::take_plot() {
            if self.sessions.iter().any(|s| s.id == id) {
                self.switch_session(id, window, cx);
                self.page = "serial";
                self.serial_page = "plot";
                self.plotting = true;
                let failure = self
                    .open_panel(super::auxiliary::Panel::Plot(id), cx)
                    .err()
                    .map(|e| e.to_string());
                service::ipc::finish_plot(request, failure);
            } else {
                service::ipc::finish_plot(request, Some("会话不属于当前窗口".into()));
            }
        }
    }
}
