use super::*;

fn key_bindings(os: OperatingSystem) -> [KeyBinding; 18] {
    // KeyBinding::new requires explicit modifiers; it cannot resolve `secondary-`.
    macro_rules! binding {
        ($key:literal, $action:expr) => {
            KeyBinding::new(
                if os == OperatingSystem::Mac {
                    concat!("cmd-", $key)
                } else {
                    concat!("ctrl-", $key)
                },
                $action,
                None,
            )
        };
    }

    [
        binding!("f", Find),
        binding!("r", Refresh),
        binding!("k", ConnectTarget),
        binding!("shift-k", DisconnectTarget),
        binding!("shift-s", SaveProfile),
        binding!("enter", ExecutePrimary),
        binding!("alt-enter", ToggleAutoSend),
        binding!("shift-r", ReadMemory),
        binding!("l", ClearTerminal),
        binding!(",", ShowSettings),
        binding!("o", OpenFirmware),
        binding!("1", ShowFlash),
        binding!("2", ShowSerial),
        binding!("q", Quit),
        binding!("w", CloseWindow),
        binding!("j", ShowLogs),
        binding!("alt-m", ShowMemory),
        binding!("s", SaveSettings),
    ]
}

impl Workbench {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let bindings = key_bindings(cx.platform_info().os);
        cx.bind_keys(bindings);
        let loaded =
            model::preferences_path().and_then(|path| Ok((Preferences::load(&path)?, path)));
        let (mut preferences, path, mut message) = match loaded {
            Ok((p, path)) => (p, Some(path), "准备就绪".into()),
            Err(e) => (Preferences::default(), None, format!("{e:#}")),
        };
        preferences
            .flags
            .insert("verify".into(), preferences.flag("default_verify"));
        preferences
            .flags
            .insert("reset".into(), preferences.flag("default_reset"));
        if !preferences.fields.contains_key("roll_mib")
            && let Ok(bytes) = preferences.number("roll")
        {
            preferences.fields.insert(
                "roll_mib".into(),
                (bytes as f64 / (1024. * 1024.)).to_string(),
            );
        }
        for (key, _, default) in model::FIELDS.iter().chain(model::EDITORS.iter()) {
            preferences
                .fields
                .entry((*key).into())
                .or_insert_with(|| (*default).into());
        }
        if preferences.ipc > 0
            && let Err(error) = service::ipc::configure(preferences.ipc)
        {
            message = format!("本地控制启动失败：{error}");
        }
        if let Some(argument) = std::env::args().skip(1).find(|s| {
            std::path::Path::new(s).extension().is_some_and(|e| {
                ["elf", "axf", "hex", "bin"].contains(&e.to_string_lossy().to_lowercase().as_str())
            })
        }) {
            preferences.fields.insert("firmware".into(), argument);
        }
        let fields: BTreeMap<String, Entity<InputState>> = model::FIELDS
            .iter()
            .map(|(key, _, _)| {
                (
                    (*key).into(),
                    cx.new(|cx| {
                        InputState::new(window, cx)
                            .placeholder(if *key == "filter" { "搜索数据" } else { "" })
                            .default_value(preferences.get(key).to_owned())
                    }),
                )
            })
            .collect();
        let editors: BTreeMap<String, Entity<TextareaState>> = model::EDITORS
            .iter()
            .map(|(key, _, _)| {
                (
                    (*key).into(),
                    cx.new(|cx| {
                        TextareaState::new(window, cx)
                            .placeholder(if *key == "send" {
                                "输入要发送的数据…"
                            } else {
                                ""
                            })
                            .default_value(preferences.get(key).to_owned())
                    }),
                )
            })
            .collect();
        for field in fields.values() {
            cx.observe(field, |_, _, cx| cx.notify()).detach();
        }
        for editor in editors.values() {
            cx.observe(editor, |_, _, cx| cx.notify()).detach();
        }
        let output = cx.new(|cx| TextareaState::new(window, cx));
        let terminal = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("chipdock-terminal")
                .line_number(false)
        });
        let worker = Worker::new();
        let initializing = worker
            .sender
            .send(Command::InitializeSession(preferences.clone()))
            .is_ok();
        let scanning = worker.sender.send(Command::Scan).is_ok();
        let _ = worker.sender.send(Command::Ports);
        let firmware_output = cx.new(|cx| TextareaState::new(window, cx));
        let memory_output = cx.new(|cx| TextareaState::new(window, cx));
        let (poll_tx, requests) = mpsc::channel::<(u64, u64)>();
        let (results, poll_rx) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok((id, cursor)) = requests.recv() {
                if results
                    .send((
                        id,
                        service::serial_session_records(id, cursor, 256).map_err(Into::into),
                    ))
                    .is_err()
                {
                    break;
                }
            }
        });
        cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if view
                    .update_in(cx, |view, window, cx| view.tick(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |view, cx| {
                if view.busy_lanes.iter().any(|l| !l.background())
                    || view.sessions.iter().any(|s| {
                        s.status
                            .as_ref()
                            .is_some_and(|s| s.job_kind.is_some() || s.logging || s.capturing)
                    })
                {
                    view.message = "任务或采集正在运行，请先停止再退出".into();
                    cx.notify();
                    false
                } else {
                    view.save(window, cx);
                    view.close_panels(cx);
                    true
                }
            })
            .unwrap_or(true)
        });
        match preferences.theme.as_str() {
            "dark" => Theme::change(ThemeMode::Dark, Some(window), cx),
            "light" => Theme::change(ThemeMode::Light, Some(window), cx),
            _ => {}
        }
        super::components::native_accent(cx);
        if cfg!(target_os = "macos")
            && cx
                .text_system()
                .all_font_names()
                .iter()
                .any(|name| name == ".SF NS Mono")
        {
            Theme::global_mut(cx).mono_font_family = ".SF NS Mono".into();
        }
        Self {
            worker,
            main_window: window.window_handle(),
            auxiliary: vec![],
            installing: false,
            busy_lanes: [
                initializing.then_some(Lane::Serial),
                scanning.then_some(Lane::Probe),
            ]
            .into_iter()
            .flatten()
            .collect(),
            preferences,
            path,
            fields,
            editors,
            focus_handle,
            expanded: std::collections::BTreeSet::new(),
            connected_target: None,
            settings_tab: "general",
            page: "flash",
            serial_page: "terminal",
            message,
            logs: VecDeque::new(),
            unseen_errors: 0,
            logs_scroll: ScrollHandle::new(),
            output,
            firmware_output,
            memory_output,
            terminal,
            probes: vec![],
            chips: vec![],
            ports: vec![],
            memory: None,
            preview: None,
            firmware_key: None,
            layout: vec![],
            sessions: vec![],
            active: None,
            paused: false,
            terminal_style: String::new(),
            terminal_rows: std::rc::Rc::new(Vec::new()),
            terminal_scroll: UniformListScrollHandle::new(),
            terminal_ranges: BTreeMap::new(),
            selected_record: None,
            record_sort: 0,
            record_desc: false,
            poll_pending: std::collections::HashSet::new(),
            poll_tx,
            poll_rx,
            update: None,
            downloaded: None,
            next_update_check: std::time::Instant::now(),
            confirmation: None,
            plot_points: vec![],
            plot_cursor: 0,
            plot_revision: 0,
            plot_dropped: 0,
            plot_config: String::new(),
            plotting: false,
        }
    }
    pub(super) fn snapshot(&self, cx: &App) -> Preferences {
        let mut p = self.preferences.clone();
        for (key, state) in &self.fields {
            p.fields
                .insert(key.clone(), state.read(cx).value().to_string());
        }
        for (key, state) in &self.editors {
            p.fields
                .insert(key.clone(), state.read(cx).value().to_string());
        }
        p
    }
    pub(super) fn set(
        &mut self,
        key: &str,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = value.into();
        if let Some(field) = self.fields.get(key) {
            field.update(cx, |field, cx| field.set_value(value.clone(), window, cx));
        }
        if let Some(editor) = self.editors.get(key) {
            editor.update(cx, |editor, cx| editor.set_value(value, window, cx));
        }
    }
    pub(super) fn report_error(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        if self.page != "logs" {
            self.unseen_errors = self.unseen_errors.saturating_add(1);
        }
        self.report(format!("[错误] {}", text.into()), cx);
    }
    pub(super) fn report(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.message = text.into();
        self.logs.push_back(format!(
            "{}  {}",
            chrono::Local::now().format("%H:%M:%S"),
            self.message
        ));
        if self.preferences.flag("follow_logs") {
            self.logs_scroll
                .set_offset(point(px(0.), px(-1_000_000_000.)));
        }
        while self.logs.len() > 2000 {
            self.logs.pop_front();
        }
        cx.notify();
    }
    pub(super) fn is_busy(&self) -> bool {
        let lane = match self.page {
            "serial" => self.active.map(Lane::Session).unwrap_or(Lane::Serial),
            "settings" => Lane::Update,
            "logs" => Lane::Files,
            _ => Lane::Probe,
        };
        self.installing
            || self.busy_lanes.contains(&lane)
            || (self.page == "serial" && self.busy_lanes.contains(&Lane::Serial))
    }
    pub(super) fn submit(&mut self, command: Command, cx: &mut Context<Self>) {
        if matches!(&command, Command::InstallUpdate(..)) {
            self.plotting = false;
            for session in &mut self.sessions {
                session.plotting = false;
            }
        }
        if self.installing
            || (matches!(&command, Command::InstallUpdate(..))
                && self.busy_lanes.iter().any(|l| !l.background()))
        {
            self.report("请等待当前任务结束再安装更新", cx);
            return;
        }
        let installing = matches!(&command, Command::InstallUpdate(..));
        let lane = command.lane();
        if self.busy_lanes.contains(&lane) {
            self.report("请等待当前操作完成", cx);
            return;
        }
        if matches!(&command, Command::DownloadUpdate(_)) {
            self.worker
                .cancel_update
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }
        match self.worker.sender.send(command) {
            Ok(()) => {
                self.busy_lanes.push(lane);
                self.installing = installing;
                if !lane.background() {
                    self.message = "正在处理…".into();
                }
                cx.notify();
            }
            Err(e) => self.report_error(e.to_string(), cx),
        }
    }
    pub(super) fn request_install(
        &mut self,
        path: PathBuf,
        update: crate::update::Available,
        cx: &mut Context<Self>,
    ) {
        let p = self.snapshot(cx);
        let result = self
            .path
            .as_ref()
            .context("设置文件未成功加载，不能在丢失配置的情况下重启")
            .and_then(|path| p.save(path));
        match result {
            Ok(()) => {
                self.preferences = p;
                self.submit(Command::InstallUpdate(path, update), cx);
            }
            Err(error) => {
                self.preferences.flags.insert("auto_install".into(), false);
                self.report_error(format!("保存设置失败，已取消安装：{error:#}"), cx);
            }
        }
    }
    pub(super) fn save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let p = self.snapshot(cx);
        if let Some(path) = &self.path {
            match p.save(path) {
                Ok(()) => {
                    self.preferences = p;
                    self.report("设置已保存", cx)
                }
                Err(e) => self.report_error(format!("{e:#}"), cx),
            }
        } else {
            self.report("原设置文件读取失败，未覆盖。请先修复设置文件。", cx);
        }
    }
    pub(super) fn switch_session(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let p = self.snapshot(cx);
        if let Some(s) = self.sessions.iter_mut().find(|s| Some(s.id) == self.active) {
            s.preferences = p;
            s.plot_points = self.plot_points.clone();
            s.plot_cursor = self.plot_cursor;
            s.plotting = self.plotting;
            s.plot_revision = self.plot_revision;
            s.plot_dropped = self.plot_dropped;
            s.plot_config = self.plot_config.clone();
            s.paused = self.paused;
        }
        if let Some(s) = self.sessions.iter().find(|s| s.id == id) {
            let p = s.preferences.clone();
            self.plot_points = s.plot_points.clone();
            self.plot_cursor = s.plot_cursor;
            self.plotting = s.plotting;
            self.plot_revision = s.plot_revision;
            self.plot_dropped = s.plot_dropped;
            self.plot_config = s.plot_config.clone();
            self.paused = s.paused;
            let result = s.result.clone();
            self.output
                .update(cx, |s, cx| s.set_value(result, window, cx));
            self.preferences.history = p.history.clone();
            self.preferences.command_history = p.command_history.clone();
            for key in [
                "reconnect",
                "command_mode",
                "hex_ending",
                "auto_scroll",
                "timestamps",
                "mixed",
                "search_regex",
                "search_case",
                "reset_on_clear",
            ] {
                self.preferences.flags.insert(key.into(), p.flag(key));
            }
            for (key, _, _) in model::FIELDS.iter().skip_while(|f| f.0 != "port") {
                self.set(key, p.get(key).to_owned(), window, cx);
            }
            for (key, state) in &self.editors {
                state.update(cx, |s, cx| s.set_value(p.get(key).to_owned(), window, cx));
            }
        }
        self.active = Some(id);

        self.render_terminal(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn workbench_shortcuts_use_native_modifiers() {
        for os in [
            OperatingSystem::Mac,
            OperatingSystem::Windows,
            OperatingSystem::Linux,
        ] {
            for binding in key_bindings(os) {
                let [keystroke] = binding.keystrokes() else {
                    panic!("expected one keystroke for {binding:?}");
                };
                assert_eq!(
                    (
                        keystroke.modifiers().platform,
                        keystroke.modifiers().control
                    ),
                    (os == OperatingSystem::Mac, os != OperatingSystem::Mac),
                    "{os:?}: {binding:?}"
                );
            }
        }
    }
}
