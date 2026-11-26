//! Framework-independent desktop state and asynchronous result reconciliation.
use crate::{
    backend::{Command, Lane, Output, Worker, memory_text},
    model::{self, Preferences},
    presentation, serial,
};
use anyhow::{Context, Result, ensure};
use chipdock_core::{
    models::*,
    serial::{PollBatch, SerialFrameEvent, types::*},
};
use chipdock_service::serial as service;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

/// The update workflow independently observed by the native update dialog.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    Available,
    Current,
    Downloading { done: u64, total: u64 },
    Downloaded,
    Installing,
    Failed(String),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OperationResult {
    pub revision: u64,
    pub action: String,
    pub outcome: std::result::Result<Value, String>,
}

pub struct Session {
    pub id: u64,
    pub cursor: u64,
    pub cleared: u64,
    pub frames: VecDeque<SerialFrameEvent>,
    pub frozen: Option<Vec<SerialFrameEvent>>,
    pub status: Option<SerialStatus>,
    pub preferences: Preferences,
    pub result: String,
    pub operation_result: Option<OperationResult>,
    pub points: Vec<Value>,
    pub plot_cursor: u64,
    pub plot_revision: u64,
    pub plot_dropped: u64,
    pub plotting: bool,
}

impl Session {
    fn new(id: u64, preferences: Preferences) -> Self {
        Self {
            id,
            cursor: 0,
            cleared: 0,
            frames: VecDeque::new(),
            frozen: None,
            status: None,
            preferences,
            result: String::new(),
            operation_result: None,
            points: vec![],
            plot_cursor: 0,
            plot_revision: 0,
            plot_dropped: 0,
            plotting: false,
        }
    }
    pub fn connected(&self) -> bool {
        self.status.as_ref().is_some_and(|s| s.connected)
    }
    pub fn records(&self) -> impl Iterator<Item = &SerialFrameEvent> {
        let frozen = self.frozen.as_deref();
        frozen
            .into_iter()
            .flatten()
            .chain(self.frames.iter().filter(move |_| frozen.is_none()))
            .filter(|frame| frame.sequence > self.cleared)
    }
}

pub struct Workbench {
    pub preferences: Preferences,
    pub sessions: BTreeMap<u64, Session>,
    pub active: Option<u64>,
    pub busy: HashSet<Lane>,
    pub probes: Vec<ProbeSummary>,
    pub probes_scanned: bool,
    pub probe_scan_revision: u64,
    pub chips: Vec<String>,
    pub ports: Vec<PortInfo>,
    pub preview: Option<FirmwarePreview>,
    pub firmware_text: String,
    pub firmware_error: Option<String>,
    pub memory: Option<MemoryReadResult>,
    pub memory_text: String,
    pub layout: Vec<MemoryRegionLayout>,
    layout_key: Option<String>,
    layout_request: Option<String>,
    pub target: Option<ConnectionInfo>,
    identified: Option<String>,
    connect_request: Option<String>,
    partial_target_information: Option<(String, TargetInformation)>,
    firmware_key: Option<(String, String)>,
    preview_request: Option<(String, String)>,
    pub logs: VecDeque<String>,
    pub unseen_errors: usize,
    pub logs_visible: bool,
    pub message: String,
    pub progress: Option<f32>,
    pub output: String,
    pub update: Option<crate::update::Available>,
    pub update_state: UpdateState,
    pub downloaded: bool,
    pub installing: bool,
    pub restart: bool,
    pub plot_request: Option<u64>,
    pub worker: Worker,
    path: Option<PathBuf>,
    poll_tx: mpsc::Sender<(u64, u64)>,
    poll_rx: mpsc::Receiver<(u64, Result<PollBatch>)>,
    poll_pending: HashSet<u64>,
    next_update: Instant,
    startup: VecDeque<Command>,
    importing: Option<(Option<u64>, String)>,
    pending_workbench: BTreeMap<u64, String>,
    pending_framing: BTreeMap<u64, (String, String)>,
    pending_file_writes: usize,
}
impl Workbench {
    pub fn new() -> Self {
        let loaded = model::preferences_path().and_then(|p| Ok((Preferences::load(&p)?, p)));
        let (mut preferences, path, message) = match loaded {
            Ok((p, path)) => (p, Some(path), "准备就绪".into()),
            Err(e) => (Preferences::default(), None, format!("{e:#}")),
        };
        for key in ["verify", "reset"] {
            preferences
                .flags
                .insert(key.into(), preferences.flag(&format!("default_{key}")));
        }
        if !preferences.fields.contains_key("roll_mib")
            && let Ok(bytes) = preferences.number("roll")
        {
            preferences
                .fields
                .insert("roll_mib".into(), (bytes as f64 / 1048576.).to_string());
        }
        for (key, _, value) in model::FIELDS.iter().chain(model::EDITORS) {
            preferences
                .fields
                .entry((*key).into())
                .or_insert_with(|| (*value).into());
        }
        if let Some(path) = crate::instance::firmware_argument() {
            preferences.fields.insert("firmware".into(), path);
        }
        let mut s = Self::unstarted(preferences, path, message);
        s.startup.push_back(Command::Ports);
        if s.preferences.ipc > 0 {
            s.startup.push_back(Command::Ipc(s.preferences.ipc));
        }
        s.submit(Command::InitializeSession(s.preferences.clone()));
        s
    }
    /// Creates workers without submitting discovery, IPC or device commands.
    fn unstarted(preferences: Preferences, path: Option<PathBuf>, message: String) -> Self {
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
        Self {
            preferences,
            path,
            sessions: BTreeMap::new(),
            active: None,
            busy: HashSet::new(),
            probes: vec![],
            probes_scanned: false,
            probe_scan_revision: 0,
            chips: vec![],
            ports: vec![],
            preview: None,
            firmware_text: String::new(),
            firmware_error: None,
            memory: None,
            memory_text: String::new(),
            layout: vec![],
            layout_key: None,
            layout_request: None,
            target: None,
            identified: None,
            connect_request: None,
            partial_target_information: None,
            firmware_key: None,
            preview_request: None,
            logs: VecDeque::new(),
            unseen_errors: 0,
            logs_visible: false,
            message,
            progress: None,
            output: String::new(),
            update: None,
            update_state: UpdateState::Idle,
            downloaded: false,
            installing: false,
            restart: false,
            plot_request: None,
            worker: Worker::new(),
            poll_tx,
            poll_rx,
            poll_pending: HashSet::new(),
            next_update: Instant::now(),
            startup: VecDeque::new(),
            importing: None,
            pending_workbench: BTreeMap::new(),
            pending_framing: BTreeMap::new(),
            pending_file_writes: 0,
        }
    }
    pub fn active_session(&self) -> Option<&Session> {
        self.active.and_then(|id| self.sessions.get(&id))
    }
    pub fn target_connected(&self) -> bool {
        self.identified
            .as_ref()
            .is_some_and(|key| key == &self.preferences.target_key())
    }
    pub fn target_connecting(&self) -> bool {
        self.connect_request
            .as_ref()
            .is_some_and(|key| key == &self.preferences.target_key())
    }
    pub fn target_information(&self) -> Option<&TargetInformation> {
        if let Some((key, information)) = &self.partial_target_information
            && key == &self.preferences.target_key()
        {
            return Some(information);
        }
        self.target_connected()
            .then_some(self.target.as_ref())
            .flatten()
            .and_then(|target| target.target_information.as_ref())
    }
    pub fn target_name(&self) -> String {
        if let Some(information) = self.target_information() {
            return information.device_type.clone();
        }
        if self.target_connected()
            && let Some(target) = &self.target
        {
            return target.chip.clone();
        }
        let chip = self.preferences.get("chip");
        if chip.is_empty() {
            "自动识别".into()
        } else {
            chip.into()
        }
    }
    pub fn firmware_analyzing(&self) -> bool {
        self.preview_request.as_ref().is_some_and(|(path, base)| {
            path == self.preferences.get("firmware") && base == self.preferences.get("base")
        })
    }
    pub fn serial_connected(&self) -> bool {
        self.active_session().is_some_and(Session::connected)
    }
    pub fn lane_busy(&self, lane: Lane) -> bool {
        self.installing || self.busy.contains(&lane)
    }
    pub fn serial_busy(&self) -> bool {
        self.lane_busy(Lane::Serial)
            || self
                .active
                .is_none_or(|id| self.lane_busy(Lane::Session(id)))
    }
    pub fn can_quit(&self) -> bool {
        self.pending_file_writes == 0
            && self.busy.iter().all(|l| l.background())
            && self.sessions.values().all(|s| {
                s.status
                    .as_ref()
                    .is_none_or(|s| s.job_kind.is_none() && !s.logging && !s.capturing)
            })
    }
    pub fn begin_file_write(&mut self) -> bool {
        if self.installing {
            return false;
        }
        self.pending_file_writes += 1;
        true
    }
    pub fn finish_file_write(&mut self) {
        self.pending_file_writes = self.pending_file_writes.saturating_sub(1);
    }
    pub fn can_install_update(&self) -> bool {
        self.can_quit()
            && self.sessions.values().all(|session| {
                session.status.as_ref().is_some_and(|status| {
                    !status.connected
                        && !status.reconnecting
                        && status.job_kind.is_none()
                        && !status.logging
                        && !status.capturing
                })
            })
    }
    pub fn cancel_update(&mut self) {
        if matches!(
            self.update_state,
            UpdateState::Checking | UpdateState::Downloading { .. }
        ) {
            let checking = matches!(self.update_state, UpdateState::Checking);
            self.worker
                .cancel_update
                .store(true, std::sync::atomic::Ordering::Relaxed);
            self.update_state = UpdateState::Cancelled;
            self.background_report(if checking {
                "检查更新已取消"
            } else {
                "下载已取消"
            });
        }
    }
    pub fn set(&mut self, key: &str, value: String) {
        if self.installing {
            return;
        }
        if self.preferences.get(key) == value {
            return;
        }
        self.preferences.fields.insert(key.into(), value);
        if matches!(key, "firmware" | "base") {
            self.preview = None;
            self.firmware_error = None;
            self.firmware_text.clear();
        }
        if matches!(key, "chip" | "probe" | "wire" | "speed") {
            self.invalidate_target();
        }
        if key == "plot"
            && let Some(s) = self.active.and_then(|id| self.sessions.get_mut(&id))
        {
            s.plot_revision += 1;
            s.points.clear();
            s.plot_dropped = 0;
        }
    }
    pub fn flag(&mut self, key: &str, value: bool) {
        if !self.installing {
            let changed = self.preferences.flag(key) != value;
            self.preferences.flags.insert(key.into(), value);
            if changed && key == "under_reset" {
                self.invalidate_target();
            }
        }
    }
    pub fn switch_session(&mut self, id: u64) {
        if self.installing || self.active == Some(id) || !self.sessions.contains_key(&id) {
            return;
        }
        self.stash_session();
        if let Some(s) = self.sessions.get(&id) {
            restore_serial_preferences(&mut self.preferences, &s.preferences);
            self.output = s.result.clone();
            self.active = Some(id);
        }
    }
    fn stash_session(&mut self) {
        if let Some(s) = self.active.and_then(|id| self.sessions.get_mut(&id)) {
            s.preferences = self.preferences.clone();
        }
    }
    pub fn clear_terminal(&mut self) {
        let reset = self.preferences.flag("reset_on_clear");
        if let Some(s) = self.active.and_then(|id| self.sessions.get_mut(&id)) {
            s.cleared = s.cursor;
            s.frames.clear();
            if s.frozen.is_some() {
                s.frozen = Some(vec![]);
            }
            if reset {
                self.serial_action("resetCounters");
            }
        }
    }
    pub fn pause(&mut self) {
        if let Some(s) = self.active.and_then(|id| self.sessions.get_mut(&id)) {
            s.frozen = if s.frozen.is_some() {
                None
            } else {
                Some(s.frames.iter().cloned().collect())
            };
        }
    }
    pub fn report(&mut self, text: impl Into<String>) {
        self.message = text.into();
        self.append_log(self.message.clone());
    }
    pub fn set_logs_visible(&mut self, visible: bool) {
        self.logs_visible = visible;
        if visible {
            self.unseen_errors = 0;
        }
    }
    pub fn select_scanned_probe(&mut self) {
        if let Some(probe) = scanned_probe_selection(self.preferences.get("probe"), &self.probes) {
            let invalid = probe.is_empty();
            self.set("probe", probe);
            if invalid {
                self.invalidate_target();
            }
        }
    }
    fn append_log(&mut self, text: String) {
        if text.starts_with("[错误]") && !self.logs_visible {
            self.unseen_errors = self.unseen_errors.saturating_add(1);
        }
        self.logs.push_back(format!(
            "{}  {}",
            chrono::Local::now().format("%H:%M:%S"),
            text
        ));
        while self.logs.len() > 2000 {
            self.logs.pop_front();
        }
    }
    pub fn error(&mut self, error: impl std::fmt::Display) {
        self.report(format!("[错误] {error}"));
    }
    pub fn submit(&mut self, command: Command) {
        let lane = command.lane();
        if self.installing || self.busy.contains(&lane) {
            if lane != Lane::Registry {
                self.report("请等待当前操作完成");
            }
            return;
        }
        if matches!(&command, Command::InstallUpdate) {
            if !self.can_install_update() {
                self.report("请先断开全部串口并停止任务和采集，再安装更新");
                return;
            }
            self.installing = true;
            for s in self.sessions.values_mut() {
                s.plotting = false;
                s.plot_revision += 1;
            }
        }
        if matches!(&command, Command::CheckUpdate | Command::DownloadUpdate(_)) {
            self.worker
                .cancel_update
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }
        let update_state = match &command {
            Command::CheckUpdate => Some(UpdateState::Checking),
            Command::DownloadUpdate(update) => Some(UpdateState::Downloading {
                done: 0,
                total: update.asset.size.unwrap_or_default(),
            }),
            Command::InstallUpdate => Some(UpdateState::Installing),
            _ => None,
        };
        let connect_request = if let Command::Connect(preferences) = &command {
            Some(preferences.target_key())
        } else {
            None
        };
        let preview_request = if let Command::Preview(preferences) = &command {
            Some((
                preferences.get("firmware").to_owned(),
                preferences.get("base").to_owned(),
            ))
        } else {
            None
        };
        let layout_request = if let Command::Layout(preferences) = &command {
            Some(preferences.get("chip").to_owned())
        } else {
            None
        };
        let importing = if let Command::Import(key, _) = &command {
            Some((self.active, key.clone()))
        } else {
            None
        };
        let workbench_action = if let Command::Workbench(id, request) = &command {
            Some((
                *id,
                request
                    .get("action")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ))
        } else {
            None
        };
        let is_preview = preview_request.is_some();
        let is_workbench = workbench_action.is_some();
        if connect_request.is_some() {
            // Release the previous watch before the worker can install the new one.
            self.invalidate_target();
        }
        match self.worker.sender.send(command) {
            Ok(()) => {
                if let Some((id, action)) = workbench_action {
                    self.pending_workbench.insert(id, action);
                }
                if let Some(request) = connect_request {
                    self.connect_request = Some(request);
                }
                if let Some(request) = preview_request {
                    self.preview_request = Some(request);
                    self.firmware_error = None;
                }
                if let Some(request) = layout_request {
                    if self.layout_key.as_ref() != Some(&request) {
                        self.layout.clear();
                    }
                    self.layout_key = Some(request.clone());
                    self.layout_request = Some(request);
                }
                if let Some(update_state) = update_state {
                    if matches!(update_state, UpdateState::Downloading { .. }) {
                        self.downloaded = false;
                    }
                    self.update_state = update_state;
                }
                if importing.is_some() {
                    self.importing = importing;
                }
                self.busy.insert(lane);
                if !is_preview
                    && !is_workbench
                    && !lane.background()
                    && (lane == Lane::Probe || !self.busy.contains(&Lane::Probe))
                {
                    self.message = "正在处理…".into();
                }
            }
            Err(e) => {
                if let Some((id, action)) = workbench_action {
                    if let Some(session) = self.sessions.get_mut(&id) {
                        let revision = session
                            .operation_result
                            .as_ref()
                            .map_or(1, |previous| previous.revision.saturating_add(1));
                        let message = e.to_string();
                        session.result = message.clone();
                        session.operation_result = Some(OperationResult {
                            revision,
                            action,
                            outcome: Err(message),
                        });
                    }
                    return;
                }
                if preview_request.is_some() {
                    let message = e.to_string();
                    self.firmware_error = Some(message.clone());
                    self.append_log(format!("[错误] {message}"));
                    return;
                }
                if let Some(request) = layout_request {
                    self.layout_key = Some(request);
                    self.append_log(format!("[错误] 内存映射：{e}"));
                    return;
                }
                self.installing = false;
                if lane == Lane::Update {
                    self.update_state = UpdateState::Failed(e.to_string());
                }
                self.error(e);
            }
        }
    }
    pub fn submit_framing(&mut self, id: u64, request: Value, mode: String, value: String) -> bool {
        if request.get("action").and_then(Value::as_str) != Some("framing")
            || self.installing
            || self.lane_busy(Lane::Session(id))
            || !self.sessions.contains_key(&id)
        {
            return false;
        }
        self.submit(Command::Workbench(id, request));
        if self.pending_workbench.contains_key(&id) {
            self.pending_framing.insert(id, (mode, value));
            true
        } else {
            false
        }
    }
    pub fn serial_action(&mut self, action: &str) {
        let Some(id) = self.active else {
            return;
        };
        match serial::request(&self.preferences, action) {
            Ok(mut value) => {
                if action == "export"
                    && let Some(s) = self.sessions.get(&id)
                {
                    value["after"] = json!(s.cleared);
                }
                self.submit(Command::Workbench(id, value));
            }
            Err(e) => self.error(e),
        }
    }
    pub fn connect_serial(&mut self) {
        if let Some(id) = self.active {
            let connected = self
                .active_session()
                .and_then(|s| s.status.as_ref())
                .is_some_and(|s| s.connected || s.reconnecting);
            self.submit(if connected {
                Command::Disconnect(id)
            } else {
                Command::ConnectSerial(id, self.preferences.clone())
            });
        }
    }
    pub fn send(&mut self) {
        if self.serial_busy() || !self.serial_connected() {
            return;
        }
        if self.preferences.flag("command_mode") {
            self.serial_action("command");
        } else if let Some(id) = self.active {
            self.submit(Command::Send(id, self.preferences.clone()));
        }
    }
    pub fn save(&mut self) -> Result<()> {
        self.stash_session();
        self.preferences.save(
            self.path
                .as_ref()
                .context("原设置文件读取失败，未覆盖；请先修复设置文件")?,
        )
    }
    pub fn request_install(&mut self) {
        if !self.can_install_update() {
            self.report("请先断开全部串口并停止任务和采集，再安装更新");
            return;
        }
        if self.downloaded && self.update.as_ref().is_some_and(|u| u.can_install) {
            match self.save() {
                Ok(()) => self.submit(Command::InstallUpdate),
                Err(e) => {
                    self.preferences.flags.insert("auto_install".into(), false);
                    let message = format!("保存设置失败，已取消安装：{e:#}");
                    self.update_state = UpdateState::Failed(message.clone());
                    self.error(message);
                }
            }
        }
    }
    pub fn tick(&mut self) -> bool {
        let mut changed = self.refresh_layout();
        if !self.lane_busy(Lane::Probe) {
            match crate::instance::take_firmware() {
                Ok(Some(p)) => {
                    self.set("firmware", p.to_string_lossy().into_owned());
                    changed = true;
                }
                Ok(None) => {}
                Err(e) => {
                    self.error(e);
                    changed = true;
                }
            }
            let key = (
                self.preferences.get("firmware").to_owned(),
                self.preferences.get("base").to_owned(),
            );
            if self.firmware_key.as_ref() != Some(&key) {
                self.firmware_key = Some(key.clone());
                self.preview = None;
                self.firmware_error = None;
                self.firmware_text.clear();
                changed = true;
                if !key.0.is_empty() {
                    self.submit(Command::Preview(self.preferences.clone()));
                }
            }
        }
        if self.preferences.flag("auto_update")
            && Instant::now() >= self.next_update
            && !self.lane_busy(Lane::Update)
        {
            self.next_update = Instant::now() + Duration::from_secs(86400);
            self.submit(Command::CheckUpdate);
            changed = true;
        }
        while let Ok((done, total)) = self.worker.update_progress.try_recv() {
            if matches!(self.update_state, UpdateState::Downloading { .. })
                && self.busy.contains(&Lane::Update)
            {
                self.update_state = UpdateState::Downloading { done, total };
                if !self.busy.contains(&Lane::Probe) {
                    self.message = format!("下载更新：{done} / {total} 字节");
                }
                changed = true;
            }
        }
        for event in self.worker.core.take_events() {
            let message = event.message;
            self.report(format!(
                "{}{}",
                message,
                event
                    .progress
                    .map(|p| format!("  {:.0}%", p * 100.))
                    .unwrap_or_default()
            ));
            self.message = message;
            self.progress = event.progress;
            changed = true;
        }
        while let Ok((lane, result)) = self.worker.receiver.try_recv() {
            self.finish_result(lane, result);
            changed = true;
        }
        changed |= self.dispatch_startup();
        while let Ok((id, result)) = self.poll_rx.try_recv() {
            changed |= self.accept_poll(id, result);
        }
        let mut plots = vec![];
        let mut invalid_plots = vec![];
        for s in self.sessions.values() {
            if !self.poll_pending.contains(&s.id) && self.poll_tx.send((s.id, s.cursor)).is_ok() {
                self.poll_pending.insert(s.id);
            }
            if s.plotting && !self.lane_busy(Lane::Plot(s.id)) {
                let p = if self.active == Some(s.id) {
                    &self.preferences
                } else {
                    &s.preferences
                };
                match serde_json::from_str::<Value>(p.get("plot")) {
                    Ok(config) => plots.push(Command::Plot(
                        s.id,
                        s.plot_revision,
                        json!({"action":"plot","config":config,"cursor":s.plot_cursor}),
                    )),
                    Err(error) => invalid_plots.push((s.id, error.to_string())),
                }
            }
        }
        for (id, error) in invalid_plots {
            if let Some(session) = self.sessions.get_mut(&id) {
                session.plotting = false;
                session.result = error.clone();
            }
            self.background_report(format!("[错误] 会话 {id} 曲线配置无效：{error}"));
            changed = true;
        }
        for command in plots {
            self.submit(command);
        }
        if let Some((request, id)) = service::ipc::take_plot() {
            let failure = if let Some(s) = self.sessions.get_mut(&id) {
                s.plotting = true;
                self.plot_request = Some(id);
                changed = true;
                None
            } else {
                Some("会话不属于当前窗口".into())
            };
            service::ipc::finish_plot(request, failure);
        }
        if self.automatic_install_ready() {
            self.request_install();
            changed = true;
        }
        changed
    }
    fn effective_layout_chip(&self) -> String {
        if self.target_connected()
            && let Some(target) = &self.target
        {
            target.chip.clone()
        } else {
            self.preferences.get("chip").to_owned()
        }
    }
    fn refresh_layout(&mut self) -> bool {
        let chip = self.effective_layout_chip();
        if self.layout_key.as_deref() == Some(chip.as_str()) {
            return false;
        }
        let mut changed = !self.layout.is_empty();
        self.layout.clear();
        if chip.is_empty() {
            self.layout_key = Some(chip);
            return true;
        }
        if !self.lane_busy(Lane::Registry) {
            let mut preferences = self.preferences.clone();
            preferences.fields.insert("chip".into(), chip);
            self.submit(Command::Layout(preferences));
            changed = true;
        }
        changed
    }
    fn should_automate_update(&self) -> bool {
        self.update
            .as_ref()
            .is_some_and(|update| self.preferences.get("skipped_update_version") != update.version)
    }
    fn automatic_install_ready(&self) -> bool {
        matches!(self.update_state, UpdateState::Downloaded)
            && self.preferences.flag("auto_install")
            && self.should_automate_update()
            && self.update.as_ref().is_some_and(|u| u.can_install)
            && self.downloaded
            && self.can_install_update()
    }
    fn dispatch_startup(&mut self) -> bool {
        if self.installing || self.busy.contains(&Lane::Serial) {
            return false;
        }
        let Some(command) = self.startup.pop_front() else {
            return false;
        };
        self.submit(command);
        true
    }
    fn finish_result(&mut self, lane: Lane, result: Result<Output>) {
        if let Lane::Session(id) = lane
            && let Some(action) = self.pending_workbench.remove(&id)
        {
            self.busy.remove(&lane);
            let framing = self.pending_framing.remove(&id);
            let outcome = match &result {
                Ok(Output::Workbench(owner, value)) if *owner == id => Some(Ok(value.clone())),
                Err(error) => Some(Err(operation_error_message(error, false))),
                _ => None,
            };
            if let Some(outcome) = outcome
                && let Some(session) = self.sessions.get_mut(&id)
            {
                if outcome.is_ok()
                    && let Some((mode, value)) = framing
                {
                    session
                        .preferences
                        .fields
                        .insert("frame_mode".into(), mode.clone());
                    session
                        .preferences
                        .fields
                        .insert("frame_value".into(), value.clone());
                    if self.active == Some(id) {
                        self.preferences.fields.insert("frame_mode".into(), mode);
                        self.preferences.fields.insert("frame_value".into(), value);
                    }
                }
                let revision = session
                    .operation_result
                    .as_ref()
                    .map_or(1, |previous| previous.revision.saturating_add(1));
                if let Err(message) = &outcome {
                    session.result = message.clone();
                }
                session.operation_result = Some(OperationResult {
                    revision,
                    action,
                    outcome,
                });
                if let Ok(output) = result {
                    self.accept(output);
                }
                return;
            }
            return;
        }
        let foreground_message = self
            .busy
            .iter()
            .any(|pending| *pending != lane && !pending.background())
            .then(|| self.message.clone());
        self.reconcile_result(lane, result);
        if let Some(message) = foreground_message {
            self.message = message;
        }
    }
    fn reconcile_result(&mut self, lane: Lane, result: Result<Output>) {
        self.busy.remove(&lane);
        if lane == Lane::Registry {
            let Some(request) = self.layout_request.take() else {
                return;
            };
            if request != self.effective_layout_chip() || self.layout_key.as_ref() != Some(&request)
            {
                return;
            }
            match result {
                Ok(Output::Layout(layout)) => self.layout = layout,
                Err(error) => {
                    self.layout.clear();
                    self.append_log(format!("[错误] 内存映射：{error:#}"));
                }
                Ok(_) => {}
            }
            return;
        }
        if lane == Lane::Probe {
            self.progress = None;
        }
        if lane == Lane::Probe
            && let Some(request) = self.preview_request.take()
        {
            let current = (
                self.preferences.get("firmware").to_owned(),
                self.preferences.get("base").to_owned(),
            );
            if request != current {
                return;
            }
            match result {
                Ok(output) => self.accept(output),
                Err(error) => {
                    let message = operation_error_message(&error, true);
                    self.firmware_error = Some(message.clone());
                    self.append_log(format!("[错误] {message}"));
                }
            }
            return;
        }
        let connecting = if lane == Lane::Probe {
            self.connect_request.take()
        } else {
            None
        };
        if connecting
            .as_ref()
            .is_some_and(|key| key != &self.preferences.target_key())
        {
            self.worker.core.clear_probe_watch();
            return;
        }
        let was_installing = self.installing && lane == Lane::Update;
        if lane == Lane::Update {
            self.installing = false;
        }
        if lane == Lane::Update
            && self
                .worker
                .cancel_update
                .load(std::sync::atomic::Ordering::Relaxed)
            && matches!(
                self.update_state,
                UpdateState::Cancelled | UpdateState::Checking | UpdateState::Downloading { .. }
            )
        {
            self.update_state = UpdateState::Cancelled;
            return;
        }
        match result {
            Ok(output) => {
                if let Some(message) =
                    completion_status(lane, &output, self.busy.iter().all(|l| l.background()))
                {
                    self.message = message.into();
                }
                self.accept(output)
            }
            Err(e) => {
                if let Lane::Plot(id) = lane
                    && let Some(s) = self.sessions.get_mut(&id)
                {
                    s.plotting = false;
                }
                if let Some(chipdock_service::records::CoreError::Operation { response }) =
                    e.downcast_ref::<chipdock_service::records::CoreError>()
                {
                    if connecting.is_some() {
                        self.partial_target_information = response
                            .target_information
                            .clone()
                            .map(|information| (self.preferences.target_key(), information));
                    }
                    self.chips = response
                        .target_candidates
                        .iter()
                        .map(|c| c.name.clone())
                        .collect();
                    self.output = format!(
                        "{}\n{}\n{}",
                        response.message,
                        response.detail.as_deref().unwrap_or(""),
                        response.recovery
                    );
                    self.error(self.output.clone());
                } else {
                    self.error(format!("{e:#}"));
                }
                if lane == Lane::Update {
                    self.update_state = UpdateState::Failed(format!("{e:#}"));
                }
                if lane == Lane::Files {
                    self.importing = None;
                }
                if was_installing {
                    self.preferences.flags.insert("auto_install".into(), false);
                }
            }
        }
    }
    fn accept_poll(&mut self, id: u64, result: Result<PollBatch>) -> bool {
        self.poll_pending.remove(&id);
        if !self.sessions.contains_key(&id) {
            return false;
        }
        let batch = match result {
            Ok(batch) => batch,
            Err(error) => {
                self.background_report(format!("[错误] {error:#}"));
                return true;
            }
        };
        let active = self.active == Some(id);
        let idle = !self.busy.contains(&Lane::Session(id));
        let Some(session) = self.sessions.get_mut(&id) else {
            return false;
        };
        if session
            .status
            .as_ref()
            .is_some_and(|old| old.generation > batch.status.generation)
        {
            return false;
        }
        let status_changed = session.status.as_ref().is_none_or(|old| {
            serde_json::to_value(old).ok() != serde_json::to_value(&batch.status).ok()
        });
        if idle
            && batch.status.connected
            && let Some((path, config)) =
                batch.status.path.as_ref().zip(batch.status.config.as_ref())
        {
            session.preferences.apply_connection(path, config);
            if active {
                self.preferences.apply_connection(path, config);
            }
        }
        let cursor = session.cursor.max(session.cleared);
        let before = session.frames.len();
        session.frames.extend(
            batch
                .frames
                .into_iter()
                .filter(|frame| frame.sequence > cursor),
        );
        let received = session.frames.len() != before;
        session.cursor = session.cursor.max(batch.next_cursor);
        session.status = Some(batch.status);
        trim_frames(&mut session.frames);
        if batch.dropped_frames > 0 {
            self.background_report(format!("接收缓冲区已淘汰 {} 帧", batch.dropped_frames));
        }
        status_changed || received || batch.dropped_frames > 0
    }
    fn background_report(&mut self, message: impl Into<String>) {
        let previous = self
            .busy
            .contains(&Lane::Probe)
            .then(|| self.message.clone());
        self.report(message);
        if let Some(previous) = previous {
            self.message = previous;
        }
    }
    fn accept(&mut self, output: Output) {
        match output {
            Output::Probes(v) => {
                self.probes = v;
                self.probes_scanned = true;
                self.probe_scan_revision = self.probe_scan_revision.wrapping_add(1);
            }
            Output::Chips(v) => self.chips = v,
            Output::Ports(v) => {
                self.ports = v;
                let locked = self.serial_busy()
                    || self
                        .active_session()
                        .and_then(|s| s.status.as_ref())
                        .is_some_and(|s| s.connected || s.reconnecting);
                let selected = self.preferences.get("port");
                if !locked
                    && (selected.is_empty() || !self.preferences.flag("reconnect"))
                    && !self.ports.iter().any(|port| port.path == selected)
                {
                    let first = self
                        .ports
                        .first()
                        .map(|port| port.path.clone())
                        .unwrap_or_default();
                    self.set("port", first);
                }
            }
            Output::Connected(info) => {
                self.partial_target_information = None;
                self.preferences
                    .fields
                    .insert("chip".into(), info.chip.clone());
                self.preferences
                    .fields
                    .insert("probe".into(), info.probe.clone());
                self.preferences.fields.insert(
                    "wire".into(),
                    match info.protocol {
                        WireProtocol::Swd => "swd",
                        WireProtocol::Jtag => "jtag",
                    }
                    .into(),
                );
                if let Some(speed) = info.speed_khz {
                    self.preferences
                        .fields
                        .insert("speed".into(), speed.to_string());
                }
                self.preferences
                    .flags
                    .insert("under_reset".into(), info.connect_under_reset);
                self.identified = Some(self.preferences.target_key());
                self.target = Some(info);
                self.report("目标已连接");
            }
            Output::Preview(preview, input) => {
                if !self.preferences.firmware().is_ok_and(|current| {
                    current.path == input.path && current.base_address == input.base_address
                }) {
                    return;
                }
                self.firmware_error = None;
                self.firmware_text = model::hex_dump(preview.start_address, &preview.bytes);
                self.preview = Some(preview);
                self.preferences.recent_bases.insert(
                    input.path.clone(),
                    input
                        .base_address
                        .map(|a| format!("0x{a:X}"))
                        .unwrap_or_default(),
                );
                self.preferences.recent_used.insert(
                    input.path.clone(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs()),
                );
                self.preferences.recent.retain(|p| p != &input.path);
                self.preferences.recent.insert(0, input.path);
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
                    Ok(text) => self.memory_text = text,
                    Err(e) => self.error(e),
                };
                self.memory = Some(m);
            }
            Output::Layout(v) => self.layout = v,
            Output::Session(id, warning) => {
                self.stash_session();
                self.sessions
                    .insert(id, Session::new(id, self.preferences.clone()));
                self.switch_session(id);
                if let Some(warning) = warning {
                    self.error(warning);
                }
                if self.sessions.len() == 1
                    && self.preferences.flag("auto_connect")
                    && !self.preferences.get("port").is_empty()
                {
                    self.submit(Command::ConnectSerial(id, self.preferences.clone()));
                }
            }
            Output::Closed(id) => {
                self.sessions.remove(&id);
                if self.active == Some(id) {
                    self.active = None;
                    if let Some(id) = self.sessions.keys().next().copied() {
                        self.switch_session(id);
                    }
                }
            }
            Output::Serial(id, status) => {
                if let Some(session) = self.sessions.get_mut(&id) {
                    if session
                        .status
                        .as_ref()
                        .is_some_and(|old| old.generation > status.generation)
                    {
                        return;
                    }
                    if status.connected
                        && let Some((path, config)) =
                            status.path.as_ref().zip(status.config.as_ref())
                    {
                        session.preferences.apply_connection(path, config);
                        if self.active == Some(id) {
                            self.preferences.apply_connection(path, config);
                        }
                    }
                    session.status = Some(status);
                }
            }
            Output::Workbench(id, value) => {
                let text = serde_json::to_string_pretty(&value).unwrap_or_default();
                if let Some(s) = self.sessions.get_mut(&id) {
                    s.result = text.clone();
                }
                if self.active == Some(id) {
                    self.output = text;
                }
            }
            Output::Sent(id, text) => self.sent(id, text, false, None),
            Output::CommandSent(id, text, value) => self.sent(id, text, true, Some(value)),
            Output::Imported(key, text) => {
                let owner = self.importing.take();
                match owner {
                    Some((Some(id), expected)) if expected == key && self.active != Some(id) => {
                        if let Some(session) = self.sessions.get_mut(&id) {
                            session.preferences.fields.insert(key, text);
                        }
                    }
                    Some((_, expected)) if expected != key => {}
                    _ => self.set(&key, text),
                }
            }
            Output::Text(text) => {
                self.output = text.clone();
                self.report(text);
            }
            Output::Ipc(mode) => {
                self.preferences.ipc = mode;
                if let Err(e) = self.save() {
                    self.error(e);
                }
            }
            Output::Plot(id, revision, value) => {
                if let Some(s) = self.sessions.get_mut(&id)
                    && s.plotting
                    && s.plot_revision == revision
                {
                    presentation::append_plot(
                        &mut s.points,
                        &mut s.plot_cursor,
                        &mut s.plot_dropped,
                        &value,
                    );
                }
            }
            Output::Update(update) => {
                if self
                    .worker
                    .cancel_update
                    .load(std::sync::atomic::Ordering::Relaxed)
                    && matches!(
                        self.update_state,
                        UpdateState::Cancelled | UpdateState::Checking
                    )
                {
                    self.update_state = UpdateState::Cancelled;
                    return;
                }
                self.downloaded = false;
                self.update = update;
                self.update_state = if self.update.is_some() {
                    UpdateState::Available
                } else {
                    UpdateState::Current
                };
                self.report(if self.update.is_some() {
                    "发现新版本"
                } else {
                    "当前已是最新版本"
                });
                if self.preferences.flag("auto_download")
                    && self.should_automate_update()
                    && self.update.as_ref().is_some_and(|u| u.can_install)
                    && let Some(update) = self.update.clone()
                {
                    self.submit(Command::DownloadUpdate(update));
                }
            }
            Output::Downloaded => {
                if self
                    .worker
                    .cancel_update
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    self.downloaded = false;
                    self.update_state = UpdateState::Cancelled;
                    self.report("下载已取消");
                } else {
                    self.downloaded = true;
                    self.update_state = UpdateState::Downloaded;
                    self.report("更新已下载并校验，安装前请断开串口");
                }
            }
            Output::Restart => self.restart = true,
        }
    }
    fn sent(&mut self, id: u64, text: String, command: bool, result: Option<Value>) {
        let result = result.and_then(|v| serde_json::to_string_pretty(&v).ok());
        if let Some(session) = self.sessions.get_mut(&id) {
            remember(
                if command {
                    &mut session.preferences.command_history
                } else {
                    &mut session.preferences.history
                },
                &text,
            );
            if let Some(result) = &result {
                session.result = result.clone();
            }
        }
        if self.active == Some(id) {
            remember(
                if command {
                    &mut self.preferences.command_history
                } else {
                    &mut self.preferences.history
                },
                &text,
            );
            if let Some(result) = result {
                self.output = result;
            }
        }
        if self.active == Some(id) && self.busy.iter().all(|lane| lane.background()) {
            self.message = "已发送".into();
        }
    }
    pub fn check_probe_removal(&mut self) -> bool {
        if self.target.is_none() || self.lane_busy(Lane::Probe) || !self.worker.core.probe_removed()
        {
            return false;
        }
        self.invalidate_target();
        self.report("烧录器已断开连接");
        true
    }

    fn invalidate_target(&mut self) {
        if self.target.is_some() {
            self.worker.core.clear_probe_watch();
        }
        self.chips.clear();
        self.identified = None;
        self.partial_target_information = None;
        self.target = None;
        self.layout.clear();
        self.layout_key = None;
        self.memory = None;
        self.memory_text.clear();
    }
    pub fn disconnect_target(&mut self) {
        self.invalidate_target();
        self.chips.clear();
        self.report("已断开");
    }
    pub fn close_session(&mut self) {
        if self.sessions.len() > 1
            && let Some(id) = self.active
            && id != service::DEFAULT_SESSION
        {
            self.submit(Command::CloseSession(id));
        }
    }
    pub fn open_recent(&mut self, path: &str) {
        let base = self
            .preferences
            .recent_bases
            .get(path)
            .cloned()
            .unwrap_or_default();
        self.set("firmware", path.into());
        self.set("base", base);
    }
    pub fn store_profile(&mut self) -> Result<()> {
        let name = self.preferences.get("profile").trim();
        ensure!(!name.is_empty(), "配置名称不能为空");
        let name = self.preferences.unique_profile_name(name);
        self.preferences.profiles.insert(
            name,
            model::Profile {
                target: self.preferences.target()?,
                probe: model::optional(self.preferences.get("probe")),
                options: self.preferences.options(),
                base: self.preferences.get("base").into(),
            },
        );
        self.save()
    }
    pub fn apply_profile(&mut self, name: &str) {
        if self.lane_busy(Lane::Probe) {
            return;
        }
        if let Some(p) = self.preferences.profiles.get(name).cloned() {
            for (key, value) in [
                ("chip", p.target.chip.unwrap_or_default()),
                ("probe", p.probe.unwrap_or_default()),
                ("base", p.base),
                (
                    "wire",
                    match p.target.protocol {
                        WireProtocol::Swd => "swd",
                        WireProtocol::Jtag => "jtag",
                    }
                    .into(),
                ),
                ("speed", p.target.speed_khz.unwrap_or(4000).to_string()),
                (
                    "erase",
                    if p.options.skip_erase {
                        "skip"
                    } else if p.options.allow_erase_all {
                        "full"
                    } else {
                        "automatic"
                    }
                    .into(),
                ),
            ] {
                self.set(key, value);
            }
            for (key, value) in [
                ("under_reset", p.target.connect_under_reset),
                ("verify", p.options.verify),
                ("reset", p.options.reset_after),
                ("dry_run", p.options.dry_run),
            ] {
                self.flag(key, value);
            }
        }
    }
}
pub fn scanned_probe_selection(current: &str, probes: &[ProbeSummary]) -> Option<String> {
    if probes.len() == 1 {
        Some(probes[0].identifier.clone())
    } else if !probes.iter().any(|probe| probe.identifier == current) {
        Some(String::new())
    } else {
        None
    }
}

fn operation_error_message(error: &anyhow::Error, prefer_detail: bool) -> String {
    if let Some(chipdock_service::records::CoreError::Operation { response }) =
        error.downcast_ref::<chipdock_service::records::CoreError>()
    {
        if prefer_detail && let Some(detail) = &response.detail {
            return detail.clone();
        }
        return response.message.clone();
    }
    format!("{error:#}")
}

fn completion_status(lane: Lane, output: &Output, idle: bool) -> Option<&'static str> {
    if lane.background() || !idle {
        return None;
    }
    Some(
        if matches!(
            output,
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
fn remember(history: &mut Vec<String>, text: &str) {
    history.retain(|s| s != text);
    history.insert(0, text.into());
    history.truncate(50);
}
fn restore_serial_preferences(target: &mut Preferences, source: &Preferences) {
    for (key, _, _) in model::FIELDS
        .iter()
        .skip_while(|f| f.0 != "port")
        .chain(model::EDITORS)
    {
        target.fields.insert((*key).into(), source.get(key).into());
    }
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
        target.flags.insert(key.into(), source.flag(key));
    }
    target.history = source.history.clone();
    target.command_history = source.command_history.clone();
}
fn trim_frames(frames: &mut VecDeque<SerialFrameEvent>) {
    let size = |f: &SerialFrameEvent| {
        f.bytes.len()
            + f.hex.len()
            + f.text.utf8.len()
            + f.text.gbk.len()
            + f.text.ascii.len()
            + f.text.latin1.len()
    };
    let mut bytes: usize = frames.iter().map(size).sum();
    while frames.len() > 2000 || bytes > 4 * 1024 * 1024 {
        if let Some(f) = frames.pop_front() {
            bytes = bytes.saturating_sub(size(&f));
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn switching_serial_state_preserves_firmware_and_distinct_drafts() {
        let mut a = Preferences::default();
        a.fields.insert("firmware".into(), "board.hex".into());
        a.fields.insert("send".into(), "first".into());
        let mut b = Preferences::default();
        b.fields.insert("send".into(), "second".into());
        b.fields.insert("baud".into(), "9600".into());
        restore_serial_preferences(&mut a, &b);
        assert_eq!(a.get("firmware"), "board.hex");
        assert_eq!(a.get("send"), "second");
        assert_eq!(a.get("baud"), "9600");
    }
    #[test]
    fn histories_remain_deduplicated_and_bounded() {
        let mut h = vec![];
        for i in 0..80 {
            remember(&mut h, &i.to_string());
        }
        remember(&mut h, "79");
        assert_eq!(h.len(), 50);
        assert_eq!(h[0], "79");
    }
    fn state() -> (Workbench, mpsc::Receiver<Command>) {
        // Keep every command observable without executing discovery, IPC, networking or hardware.
        let mut state = Workbench::unstarted(Preferences::default(), None, "准备就绪".into());
        let (sender, commands) = mpsc::channel();
        state.worker.sender = sender;
        (state, commands)
    }
    fn status(generation: u64, connected: bool) -> SerialStatus {
        serde_json::from_value(json!({
            "connected": connected, "jobDone": 0, "jobTotal": 0, "uptimeMs": 0,
            "rxPerSecond": 0, "txPerSecond": 0, "logging": false, "capturing": false,
            "reconnecting": false, "generation": generation, "rxBytes": 0, "txBytes": 0
        }))
        .unwrap()
    }
    fn frame(sequence: u64) -> SerialFrameEvent {
        serde_json::from_value(json!({"direction":"receive","bytes":[65],"hex":"41",
            "text":{"ascii":"A","gbk":"A","utf-8":"A","latin-1":"A"},
            "timestamp":"2026-11-14T00:00:00Z","source":"serial","sequence":sequence,"parsed":[]}))
        .unwrap()
    }
    fn session(state: &mut Workbench, id: u64) {
        state
            .sessions
            .insert(id, Session::new(id, state.preferences.clone()));
    }
    fn available(version: &str) -> crate::update::Available {
        crate::update::Available {
            version: version.into(),
            notes: "Release notes".into(),
            can_install: true,
            asset: nsh_updater::Available::default(),
            github: false,
        }
    }
    fn preview() -> FirmwarePreview {
        FirmwarePreview {
            usage: FirmwareUsage {
                format: FirmwareFormat::Bin,
                used_bytes: 1,
                span_bytes: 1,
                start_address: Some(0x8000),
                end_address: Some(0x8001),
                segments: vec![],
            },
            start_address: 0x8000,
            bytes: vec![Some(65)],
        }
    }
    #[test]
    fn startup_initializes_the_default_session_then_queues_ports_and_ipc_once() {
        let (mut state, commands) = state();
        state.preferences.ipc = 1;
        state.startup.extend([Command::Ports, Command::Ipc(1)]);
        state.submit(Command::InitializeSession(state.preferences.clone()));
        assert!(matches!(
            commands.try_recv().unwrap(),
            Command::InitializeSession(_)
        ));
        assert!(!state.dispatch_startup());
        state.finish_result(
            Lane::Serial,
            Ok(Output::Session(service::DEFAULT_SESSION, None)),
        );
        assert_eq!(state.active, Some(service::DEFAULT_SESSION));
        assert_eq!(state.sessions.len(), 1);
        assert!(state.dispatch_startup());
        assert!(matches!(commands.try_recv().unwrap(), Command::Ports));
        assert!(!state.dispatch_startup());
        state.finish_result(Lane::Serial, Ok(Output::Ports(vec![])));
        assert!(state.dispatch_startup());
        assert!(matches!(commands.try_recv().unwrap(), Command::Ipc(1)));
        assert!(state.startup.is_empty());
        assert!(state.logs.iter().all(|line| !line.contains("请等待")));
    }
    #[test]
    fn session_drafts_results_histories_and_imports_remain_owned_by_their_session() {
        let (mut state, commands) = state();
        state.accept(Output::Session(1, None));
        state.set("send", "first draft".into());
        state.accept(Output::Workbench(1, json!({"result":"first result"})));
        state.accept(Output::Session(2, None));
        assert!(state.output.is_empty());
        assert!(state.sessions[&1].result.contains("first result"));
        state.set("send", "second draft".into());
        state.accept(Output::CommandSent(
            1,
            "motor-speed 45".into(),
            json!({"hex":"AA BB"}),
        ));
        state.switch_session(1);
        assert_eq!(state.preferences.get("send"), "first draft");
        assert!(state.output.contains("AA BB"));
        assert_eq!(state.preferences.command_history, ["motor-speed 45"]);
        state.submit(Command::Import("packets".into(), "unused.json".into()));
        assert!(matches!(commands.try_recv().unwrap(), Command::Import(..)));
        state.switch_session(2);
        let second = state.preferences.get("packets").to_owned();
        state.finish_result(
            Lane::Files,
            Ok(Output::Imported("packets".into(), "[]".into())),
        );
        assert_eq!(state.preferences.get("send"), "second draft");
        assert_eq!(state.preferences.get("packets"), second);
        assert_eq!(state.sessions[&1].preferences.get("packets"), "[]");
        assert!(state.sessions[&2].preferences.command_history.is_empty());
    }
    #[test]
    fn paused_clear_and_replayed_batches_cannot_restore_cleared_frames() {
        let (mut state, _commands) = state();
        session(&mut state, 1);
        state.active = Some(1);
        state.accept_poll(
            1,
            Ok(PollBatch {
                status: status(1, false),
                frames: (1..=10).map(frame).collect(),
                dropped_frames: 0,
                next_cursor: 10,
                has_more: false,
            }),
        );
        state.pause();
        state.clear_terminal();
        assert_eq!(state.sessions[&1].cleared, 10);
        state.accept_poll(
            1,
            Ok(PollBatch {
                status: status(1, false),
                frames: (9..=12).map(frame).collect(),
                dropped_frames: 0,
                next_cursor: 12,
                has_more: false,
            }),
        );
        assert_eq!(state.sessions[&1].records().count(), 0);
        state.pause();
        assert_eq!(
            state.sessions[&1]
                .records()
                .map(|f| f.sequence)
                .collect::<Vec<_>>(),
            [11, 12]
        );
        state.accept_poll(
            1,
            Ok(PollBatch {
                status: status(1, false),
                frames: vec![frame(11)],
                dropped_frames: 0,
                next_cursor: 11,
                has_more: false,
            }),
        );
        assert_eq!(state.sessions[&1].cursor, 12);
        assert_eq!(state.sessions[&1].frames.len(), 2);
    }
    #[test]
    fn serial_export_uses_the_services_after_cursor() {
        let (mut state, commands) = state();
        session(&mut state, 1);
        state.active = Some(1);
        state.sessions.get_mut(&1).unwrap().cleared = 42;
        state.serial_action("export");
        let Command::Workbench(1, request) = commands.try_recv().unwrap() else {
            panic!("expected export")
        };
        assert_eq!(request["after"], 42);
        assert!(request.get("cursor").is_none());
    }
    #[test]
    fn polling_preserves_recovered_configuration_and_rejects_old_connection_generations() {
        let (mut state, _commands) = state();
        session(&mut state, 1);
        state.active = Some(1);
        state.set("send", "unsent draft".into());
        let mut current = status(3, true);
        current.path = Some("VIRTUAL-A".into());
        current.config = Some(SerialConfig {
            baud_rate: 9600,
            ..Default::default()
        });
        state.accept(Output::Serial(1, current));
        assert_eq!(state.preferences.get("baud"), "9600");
        assert_eq!(state.preferences.get("send"), "unsent draft");
        assert!(!state.accept_poll(
            1,
            Ok(PollBatch {
                status: status(2, false),
                frames: vec![],
                dropped_frames: 0,
                next_cursor: 0,
                has_more: false
            })
        ));
        assert!(state.serial_connected());
        assert!(!state.accept_poll(99, Err(anyhow::anyhow!("closed session"))));
    }
    #[test]
    fn firmware_changes_invalidate_preview_immediately_and_stale_results_are_discarded() {
        let (mut state, _commands) = state();
        state.set("firmware", "old.bin".into());
        state.preview = Some(preview());
        state.firmware_text = "old bytes".into();
        state.set("firmware", "new.bin".into());
        assert!(state.preview.is_none());
        assert!(state.firmware_text.is_empty());
        state.accept(Output::Preview(
            preview(),
            FirmwareInput {
                path: "old.bin".into(),
                format: None,
                base_address: None,
            },
        ));
        assert!(state.preview.is_none());
        assert!(state.preferences.recent.is_empty());
        state.accept(Output::Preview(
            preview(),
            state.preferences.firmware().unwrap(),
        ));
        assert!(state.preview.is_some());
    }
    #[test]
    fn firmware_analysis_error_survives_serial_startup_status() {
        let (mut state, _commands) = state();
        state.set("firmware", "reference.bin".into());
        state.submit(Command::Preview(state.preferences.clone()));
        assert!(state.firmware_analyzing());
        state.finish_result(
            Lane::Probe,
            Err(anyhow::anyhow!("无法自动确定 BIN 烧录地址")),
        );
        assert!(!state.firmware_analyzing());
        state.finish_result(Lane::Serial, Ok(Output::Ports(vec![])));
        assert_eq!(
            state.firmware_error.as_deref(),
            Some("无法自动确定 BIN 烧录地址")
        );
        state.report("准备就绪");
        assert_eq!(
            state.firmware_error.as_deref(),
            Some("无法自动确定 BIN 烧录地址")
        );
    }

    #[test]
    fn stale_firmware_errors_are_ignored_after_file_or_base_changes() {
        let (mut state, _commands) = state();
        state.set("firmware", "old.bin".into());
        state.submit(Command::Preview(state.preferences.clone()));
        state.set("firmware", "new.bin".into());
        assert!(!state.firmware_analyzing());
        let logs = state.logs.len();
        state.finish_result(Lane::Probe, Err(anyhow::anyhow!("old file error")));
        assert!(state.firmware_error.is_none());
        assert_eq!(state.logs.len(), logs);
        state.submit(Command::Preview(state.preferences.clone()));
        state.set("base", "0x08000000".into());
        state.finish_result(Lane::Probe, Err(anyhow::anyhow!("old base error")));
        assert!(state.firmware_error.is_none());
        state.submit(Command::Preview(state.preferences.clone()));
        state.finish_result(Lane::Probe, Err(anyhow::anyhow!("current error")));
        assert_eq!(state.firmware_error.as_deref(), Some("current error"));
        state.set("base", "0x08001000".into());
        assert!(state.firmware_error.is_none());
        state.accept(Output::Preview(
            preview(),
            state.preferences.firmware().unwrap(),
        ));
        assert!(state.preview.is_some());
        assert!(state.firmware_error.is_none());
    }

    fn connected_fixture() -> ConnectionInfo {
        ConnectionInfo {
            probe: "VIRTUAL-PROBE".into(),
            chip: "STM32F103C8".into(),
            protocol: WireProtocol::Swd,
            speed_khz: Some(4000),
            connect_under_reset: false,
            target_information: Some(TargetInformation {
                device_type: "STM32F103".into(),
                device_id: Some(0x410),
                revision_id: None,
                cpu: Some("Cortex-M3".into()),
                flash_size_bytes: Some(64 * 1024),
            }),
        }
    }
    fn identification_failure() -> anyhow::Error {
        chipdock_service::records::CoreError::Operation {
            response: Box::new(chipdock_core::error::ErrorResponse {
                code: chipdock_core::error::ErrorCode::TargetIdentifyFailed,
                message: "无法唯一识别目标".into(),
                detail: None,
                target_candidates: vec![TargetCandidate {
                    name: "STM32F103C8".into(),
                    family: "STM32F1".into(),
                }],
                target_information: connected_fixture().target_information,
                recovery: "请选择识别到的目标".into(),
            }),
        }
        .into()
    }
    #[test]
    fn target_information_is_retained_for_current_identification_only() {
        let (mut state, _commands) = state();
        state.accept(Output::Connected(connected_fixture()));
        assert_eq!(state.target_name(), "STM32F103");
        assert_eq!(
            state.target_information().unwrap().cpu.as_deref(),
            Some("Cortex-M3")
        );
        state.submit(Command::Connect(state.preferences.clone()));
        assert!(state.target_connecting());
        assert!(!state.target_connected());
        assert!(state.target_information().is_none());
        state.finish_result(Lane::Probe, Err(identification_failure()));
        assert!(!state.target_connecting());
        assert!(!state.target_connected());
        assert_eq!(
            state.target_information().unwrap().flash_size_bytes,
            Some(65536)
        );
        assert_eq!(state.target_name(), "STM32F103");
        state.set("speed", "1000".into());
        assert!(state.target_information().is_none());
        state.accept(Output::Connected(connected_fixture()));
        state.disconnect_target();
        assert!(state.target_information().is_none());
    }
    #[test]
    fn old_connect_success_and_identification_error_cannot_replace_current_target() {
        let (mut state, _commands) = state();
        state.submit(Command::Connect(state.preferences.clone()));
        state.set("chip", "new-target".into());
        state.finish_result(Lane::Probe, Ok(Output::Connected(connected_fixture())));
        assert_eq!(state.preferences.get("chip"), "new-target");
        assert!(!state.target_connected());
        assert!(state.target_information().is_none());
        state.submit(Command::Connect(state.preferences.clone()));
        state.flag("under_reset", true);
        let log_count = state.logs.len();
        state.finish_result(Lane::Probe, Err(identification_failure()));
        assert!(state.target_information().is_none());
        assert!(state.chips.is_empty());
        assert_eq!(state.logs.len(), log_count);
        assert!(!state.target_connecting());
    }

    #[test]
    fn target_changes_and_disconnect_discard_old_target_memory_but_keep_firmware() {
        let (mut state, _commands) = state();
        state.set("firmware", "board.hex".into());
        state.identified = Some(state.preferences.target_key());
        state.memory = Some(MemoryReadResult {
            address: 16,
            length: 1,
            data_hex: "41".into(),
        });
        state.memory_text = "old target bytes".into();
        state.flag("under_reset", !state.preferences.flag("under_reset"));
        assert!(!state.target_connected());
        assert!(state.memory.is_none());
        assert!(state.memory_text.is_empty());
        state.memory = Some(MemoryReadResult {
            address: 16,
            length: 1,
            data_hex: "41".into(),
        });
        state.memory_text = "bytes".into();
        state.chips.push("candidate".into());
        state.disconnect_target();
        assert!(state.memory.is_none());
        assert!(state.memory_text.is_empty());
        assert!(state.chips.is_empty());
        assert_eq!(state.preferences.get("firmware"), "board.hex");
    }
    #[test]
    fn inactive_session_results_and_update_events_do_not_overwrite_probe_progress() {
        let (mut state, _commands) = state();
        session(&mut state, 1);
        session(&mut state, 2);
        state.active = Some(1);
        state.busy.insert(Lane::Probe);
        state.progress = Some(0.5);
        state.message = "正在烧录".into();
        state.finish_result(Lane::Session(2), Ok(Output::Sent(2, "background".into())));
        assert_eq!(state.message, "正在烧录");
        assert_eq!(state.progress, Some(0.5));
        assert_eq!(state.sessions[&2].preferences.history, ["background"]);
        assert!(state.preferences.history.is_empty());
        state.finish_result(Lane::Update, Err(anyhow::anyhow!("network unavailable")));
        assert_eq!(state.message, "正在烧录");
        assert!(matches!(state.update_state, UpdateState::Failed(_)));
    }
    #[test]
    fn plot_results_respect_their_session_revision_and_pause() {
        let (mut state, _commands) = state();
        session(&mut state, 1);
        session(&mut state, 2);
        state.active = Some(1);
        state.sessions.get_mut(&2).unwrap().plotting = true;
        state.sessions.get_mut(&2).unwrap().plot_revision = 2;
        let batch = json!({"nextCursor":7,"points":[{"seq":7,"values":{"value":1}}]});
        state.accept(Output::Plot(2, 1, batch.clone()));
        assert!(state.sessions[&2].points.is_empty());
        state.accept(Output::Plot(2, 2, batch.clone()));
        assert_eq!(state.sessions[&2].points.len(), 1);
        assert!(state.sessions[&1].points.is_empty());
        state.sessions.get_mut(&2).unwrap().plotting = false;
        state.accept(Output::Plot(2, 2, batch));
        assert_eq!(state.sessions[&2].points.len(), 1);
    }
    #[test]
    fn update_states_follow_accepted_requests_and_cancelled_results_cannot_start_downloads() {
        let (mut state, commands) = state();
        state.update = Some(available("1.0.0"));
        state.preferences.flags.insert("auto_download".into(), true);
        state.submit(Command::CheckUpdate);
        assert!(matches!(commands.try_recv().unwrap(), Command::CheckUpdate));
        assert_eq!(state.update_state, UpdateState::Checking);
        state.cancel_update();
        assert_eq!(state.update_state, UpdateState::Cancelled);
        state.finish_result(Lane::Update, Ok(Output::Update(Some(available("2.0.0")))));
        assert_eq!(state.update_state, UpdateState::Cancelled);
        assert_eq!(state.update.as_ref().unwrap().version, "1.0.0");
        assert!(commands.try_recv().is_err());
        state.submit(Command::CheckUpdate);
        assert!(matches!(commands.try_recv().unwrap(), Command::CheckUpdate));
        assert!(
            !state
                .worker
                .cancel_update
                .load(std::sync::atomic::Ordering::Relaxed)
        );
        state.finish_result(Lane::Update, Ok(Output::Update(None)));
        assert_eq!(state.update_state, UpdateState::Current);
        assert!(state.update.is_none());
        let update = available("2.0.0");
        state.submit(Command::DownloadUpdate(update));
        assert!(matches!(
            commands.try_recv().unwrap(),
            Command::DownloadUpdate(_)
        ));
        assert_eq!(
            state.update_state,
            UpdateState::Downloading { done: 0, total: 0 }
        );
        state.cancel_update();
        state.finish_result(Lane::Update, Ok(Output::Downloaded));
        assert_eq!(state.update_state, UpdateState::Cancelled);
        assert!(!state.downloaded);
    }
    #[test]
    fn update_install_requires_disconnected_sessions_and_a_successful_preferences_save() {
        let (mut state, commands) = state();
        session(&mut state, 1);
        state.active = Some(1);
        state.sessions.get_mut(&1).unwrap().status = Some(status(1, true));
        state.update = Some(available("2.0.0"));
        state.downloaded = true;
        state.update_state = UpdateState::Downloaded;
        state.request_install();
        assert!(commands.try_recv().is_err());
        assert_eq!(state.update_state, UpdateState::Downloaded);
        state.sessions.get_mut(&1).unwrap().status = Some(status(1, false));
        state.preferences.flags.insert("auto_install".into(), true);
        state.request_install();
        assert!(commands.try_recv().is_err());
        assert!(matches!(state.update_state, UpdateState::Failed(_)));
        assert!(!state.preferences.flag("auto_install"));
        assert!(!state.restart);
    }
    #[test]
    fn retained_and_frozen_serial_frames_stay_bounded() {
        let (mut state, _commands) = state();
        session(&mut state, 1);
        state.active = Some(1);
        state.accept_poll(
            1,
            Ok(PollBatch {
                status: status(1, false),
                frames: (1..=2002).map(frame).collect(),
                dropped_frames: 0,
                next_cursor: 2002,
                has_more: false,
            }),
        );
        assert_eq!(state.sessions[&1].frames.len(), 2000);
        assert_eq!(state.sessions[&1].frames.front().unwrap().sequence, 3);
        state.pause();
        state.accept_poll(
            1,
            Ok(PollBatch {
                status: status(1, false),
                frames: vec![frame(2003)],
                dropped_frames: 0,
                next_cursor: 2003,
                has_more: false,
            }),
        );
        assert_eq!(state.sessions[&1].frames.len(), 2000);
        assert_eq!(state.sessions[&1].records().next().unwrap().sequence, 3);
        let mut oversized = frame(2004);
        oversized.bytes.resize(4 * 1024 * 1024 + 1, 65);
        state.accept_poll(
            1,
            Ok(PollBatch {
                status: status(1, false),
                frames: vec![oversized],
                dropped_frames: 0,
                next_cursor: 2004,
                has_more: false,
            }),
        );
        assert!(state.sessions[&1].frames.is_empty());
        assert_eq!(state.sessions[&1].records().count(), 2000);
        state.pause();
        assert_eq!(state.sessions[&1].records().count(), 0);
    }
    #[test]
    fn skipping_a_verified_version_only_suppresses_automatic_work_and_install_cannot_repeat() {
        let (mut state, commands) = state();
        state.update = Some(available("2.0.0"));
        state
            .preferences
            .fields
            .insert("skipped_update_version".into(), "2.0.0".into());
        assert!(!state.should_automate_update());
        state.accept(Output::Update(Some(available("2.0.0"))));
        assert_eq!(state.update_state, UpdateState::Available);
        assert_eq!(state.update.as_ref().unwrap().version, "2.0.0");
        assert!(commands.try_recv().is_err());
        state.update = Some(available("2.1.0"));
        assert!(state.should_automate_update());
        state.downloaded = true;
        state.preferences.flags.insert("auto_install".into(), true);
        state.update_state = UpdateState::Cancelled;
        assert!(!state.automatic_install_ready());
        state.update_state = UpdateState::Installing;
        state.finish_result(Lane::Update, Ok(Output::Restart));
        assert!(state.restart);
        assert!(!state.automatic_install_ready());
    }
    #[test]
    fn refreshed_ports_choose_a_real_default_and_preserve_reconnect_identity() {
        let (mut state, _commands) = state();
        session(&mut state, 1);
        state.active = Some(1);
        let ports = || {
            vec![PortInfo {
                path: "VIRTUAL-A".into(),
                display_name: "虚拟串口 A".into(),
                manufacturer: None,
                serial_number: None,
                vid: None,
                pid: None,
            }]
        };
        state.accept(Output::Ports(ports()));
        assert_eq!(state.preferences.get("port"), "VIRTUAL-A");
        state.set("port", "removed".into());
        state.accept(Output::Ports(ports()));
        assert_eq!(state.preferences.get("port"), "VIRTUAL-A");
        state.flag("reconnect", true);
        state.set("port", "remembered-device".into());
        state.accept(Output::Ports(ports()));
        assert_eq!(state.preferences.get("port"), "remembered-device");
        state.set("port", String::new());
        state.accept(Output::Ports(ports()));
        assert_eq!(state.preferences.get("port"), "VIRTUAL-A");
        state.sessions.get_mut(&1).unwrap().status = Some(status(1, true));
        state.accept(Output::Ports(vec![]));
        assert_eq!(state.preferences.get("port"), "VIRTUAL-A");
    }
    #[test]
    fn unread_errors_clear_when_logs_are_visible_without_losing_the_log() {
        let (mut state, _commands) = state();
        state.report("ready");
        assert_eq!(state.unseen_errors, 0);
        state.error("one");
        assert_eq!(state.unseen_errors, 1);
        state.set_logs_visible(true);
        assert_eq!(state.unseen_errors, 0);
        state.error("visible");
        assert_eq!(state.unseen_errors, 0);
        state.set_logs_visible(false);
        state.error("two");
        assert_eq!(state.unseen_errors, 1);
        assert_eq!(state.logs.len(), 4);
    }
    #[test]
    fn session_operation_results_keep_revisions_errors_and_ownership() {
        let (mut state, commands) = state();
        session(&mut state, 1);
        session(&mut state, 2);
        state.active = Some(1);
        for revision in 1..=2 {
            state.submit(Command::Workbench(1, json!({"action":"checksum"})));
            assert!(matches!(commands.try_recv(), Ok(Command::Workbench(1, _))));
            state.finish_result(
                Lane::Session(1),
                Ok(Output::Workbench(1, json!({"hex":"B5 60"}))),
            );
            let result = state.sessions[&1].operation_result.as_ref().unwrap();
            assert_eq!(result.revision, revision);
            assert_eq!(result.action, "checksum");
            assert_eq!(result.outcome.as_ref().unwrap(), &json!({"hex":"B5 60"}));
            assert_eq!(state.message, "准备就绪");
        }
        state.active = Some(2);
        state.submit(Command::Workbench(1, json!({"action":"checksum"})));
        state.finish_result(
            Lane::Session(1),
            Err(anyhow::anyhow!("invalid checksum input")),
        );
        let result = state.sessions[&1].operation_result.as_ref().unwrap();
        assert_eq!(result.revision, 3);
        assert_eq!(
            result.outcome.as_ref().unwrap_err(),
            "invalid checksum input"
        );
        assert!(state.sessions[&2].operation_result.is_none());
        assert_eq!(state.message, "准备就绪");
        assert_eq!(state.unseen_errors, 0);
        state.submit(Command::Workbench(1, json!({"action":"checksum"})));
        state.sessions.remove(&1);
        state.finish_result(Lane::Session(1), Err(anyhow::anyhow!("closed session")));
        assert_eq!(state.message, "准备就绪");
        assert!(!state.busy.contains(&Lane::Session(1)));
    }
    #[test]
    fn firmware_error_uses_typed_detail_without_changing_the_activity_status() {
        let (mut state, _commands) = state();
        state.set("firmware", "firmware.bin".into());
        state.submit(Command::Preview(state.preferences.clone()));
        assert_eq!(state.message, "准备就绪");
        let error = chipdock_service::records::CoreError::Operation {
            response: Box::new(chipdock_core::error::ErrorResponse {
                code: chipdock_core::error::ErrorCode::MissingBinBaseAddress,
                message: "BIN 缺少基地址".into(),
                detail: Some("无法自动确定 BIN 烧录地址。".into()),
                target_candidates: vec![],
                target_information: None,
                recovery: "请设置 BIN 地址".into(),
            }),
        };
        state.finish_result(Lane::Probe, Err(error.into()));
        assert_eq!(
            state.firmware_error.as_deref(),
            Some("无法自动确定 BIN 烧录地址。")
        );
        assert_eq!(state.message, "准备就绪");
        assert_eq!(state.unseen_errors, 1);
        assert!(!state.logs.back().unwrap().contains("ErrorResponse"));
    }
    #[test]
    fn every_scan_reconciles_probe_choice_and_invalidates_old_candidates() {
        let (mut state, _commands) = state();
        let probe = |name: &str| ProbeSummary {
            identifier: name.into(),
            vendor_id: 0x0483,
            product_id: 0x3748,
            serial_number: None,
            product: None,
        };
        state.chips = vec!["stale candidate".into()];
        state.accept(Output::Probes(vec![probe("A")]));
        state.select_scanned_probe();
        assert_eq!(state.preferences.get("probe"), "A");
        assert!(state.chips.is_empty());
        assert_eq!(state.probe_scan_revision, 1);
        state.accept(Output::Probes(vec![probe("A")]));
        assert_eq!(state.probe_scan_revision, 2);
        assert_eq!(
            scanned_probe_selection("A", &[probe("A"), probe("B")]),
            None
        );
        state.set("probe", "missing".into());
        state.chips = vec!["old target".into()];
        state.accept(Output::Probes(vec![probe("A"), probe("B")]));
        state.select_scanned_probe();
        assert_eq!(state.preferences.get("probe"), "");
        assert!(state.chips.is_empty());
    }

    fn flash_region(size: u64) -> MemoryRegionLayout {
        MemoryRegionLayout {
            name: Some("Flash".into()),
            kind: MemoryRegionKind::Nvm,
            start: 0x0800_0000,
            end: 0x0800_0000 + size,
            size,
            cores: vec![],
            is_alias: false,
            access: MemoryAccessInfo {
                read: true,
                write: true,
                execute: true,
                boot: true,
            },
        }
    }

    #[test]
    fn offline_registry_layout_runs_while_probe_work_is_busy_without_changing_its_status() {
        let (mut state, commands) = state();
        state.set("chip", "chip-A".into());
        state.busy.insert(Lane::Probe);
        state.message = "正在烧录".into();
        state.progress = Some(0.5);
        assert!(state.refresh_layout());
        let command = commands.try_recv().unwrap();
        assert_eq!(command.lane(), Lane::Registry);
        assert!(matches!(command,Command::Layout(p)if p.get("chip")=="chip-A"));
        state.finish_result(Lane::Registry, Ok(Output::Layout(vec![flash_region(1024)])));
        assert_eq!(state.layout[0].size, 1024);
        assert_eq!(state.message, "正在烧录");
        assert_eq!(state.progress, Some(0.5));
        assert!(state.busy.contains(&Lane::Probe));
        assert!(!state.refresh_layout());
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn registry_layout_queues_only_the_latest_chip_and_discards_stale_success_or_failure() {
        let (mut state, commands) = state();
        state.set("chip", "chip-A".into());
        state.refresh_layout();
        assert!(
            matches!(commands.try_recv().unwrap(),Command::Layout(p)if p.get("chip")=="chip-A")
        );
        state.set("chip", "chip-B".into());
        state.refresh_layout();
        assert!(commands.try_recv().is_err());
        state.finish_result(Lane::Registry, Ok(Output::Layout(vec![flash_region(1)])));
        assert!(state.layout.is_empty());
        state.refresh_layout();
        assert!(
            matches!(commands.try_recv().unwrap(),Command::Layout(p)if p.get("chip")=="chip-B")
        );
        state.set("chip", "chip-C".into());
        let logs = state.logs.len();
        state.finish_result(Lane::Registry, Err(anyhow::anyhow!("old chip error")));
        assert_eq!(state.logs.len(), logs);
        state.refresh_layout();
        assert!(
            matches!(commands.try_recv().unwrap(),Command::Layout(p)if p.get("chip")=="chip-C")
        );
        state.finish_result(Lane::Registry, Ok(Output::Layout(vec![flash_region(4096)])));
        assert_eq!(state.layout[0].size, 4096);
        assert!(!state.lane_busy(Lane::Probe));
        assert!(state.can_quit());
    }

    #[test]
    fn empty_chip_and_target_invalidation_clear_layout_and_force_fresh_same_chip_lookup() {
        let (mut state, commands) = state();
        state.set("chip", "chip-A".into());
        state.refresh_layout();
        commands.try_recv().unwrap();
        state.finish_result(Lane::Registry, Ok(Output::Layout(vec![flash_region(1024)])));
        state.invalidate_target();
        assert!(state.layout.is_empty());
        assert!(state.refresh_layout());
        assert!(
            matches!(commands.try_recv().unwrap(),Command::Layout(p)if p.get("chip")=="chip-A")
        );
        state.set("chip", String::new());
        state.refresh_layout();
        state.finish_result(Lane::Registry, Ok(Output::Layout(vec![flash_region(1024)])));
        assert!(state.layout.is_empty());
        assert!(!state.refresh_layout());
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn registry_error_is_logged_once_without_replacing_status_progress_or_target_candidates() {
        let (mut state, commands) = state();
        state.set("chip", "chip-A".into());
        state.refresh_layout();
        commands.try_recv().unwrap();
        state.message = "保持原状态".into();
        state.progress = Some(0.25);
        state.chips = vec!["candidate".into()];
        state.finish_result(Lane::Registry, Err(anyhow::anyhow!("registry failure")));
        assert_eq!(state.message, "保持原状态");
        assert_eq!(state.progress, Some(0.25));
        assert_eq!(state.chips, ["candidate"]);
        assert!(state.layout.is_empty());
        assert!(
            state
                .logs
                .back()
                .unwrap()
                .contains("[错误] 内存映射：registry failure")
        );
        assert!(!state.refresh_layout());
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn local_file_writes_block_quit_and_install_until_every_write_finishes() {
        let (mut state, _commands) = state();
        assert!(state.can_quit());
        assert!(state.can_install_update());
        assert!(state.begin_file_write());
        assert!(state.begin_file_write());
        assert!(!state.can_quit());
        assert!(!state.can_install_update());
        state.finish_file_write();
        assert!(!state.can_quit());
        state.finish_file_write();
        assert!(state.can_quit());
        assert!(state.can_install_update());
        state.installing = true;
        assert!(!state.begin_file_write());
        state.installing = false;
        assert!(state.can_quit());
    }
    #[test]
    fn framing_preferences_commit_only_to_the_captured_session_after_success() {
        let (mut state, _commands) = state();
        session(&mut state, 1);
        session(&mut state, 2);
        state.active = Some(1);
        let before = state.preferences.get("frame_mode").to_owned();
        assert!(state.submit_framing(
            1,
            json!({"action":"framing","config":{"mode":"header","hex":"0d 0a"}}),
            "header".into(),
            "0d 0a".into()
        ));
        assert_eq!(state.preferences.get("frame_mode"), before);
        state.switch_session(2);
        state.finish_result(
            Lane::Session(1),
            Ok(Output::Workbench(1, json!({"ok":true}))),
        );
        assert_eq!(state.sessions[&1].preferences.get("frame_mode"), "header");
        assert_eq!(state.sessions[&1].preferences.get("frame_value"), "0d 0a");
        assert_eq!(state.preferences.get("frame_mode"), before);
        state.switch_session(1);
        assert_eq!(state.preferences.get("frame_value"), "0d 0a");
    }
    #[test]
    fn rejected_or_failed_framing_never_saves_an_unapplied_draft() {
        let (mut state, _commands) = state();
        session(&mut state, 1);
        state.active = Some(1);
        let before = state.preferences.get("frame_mode").to_owned();
        assert!(!state.submit_framing(1, json!({"action":"checksum"}), "fixed".into(), "8".into()));
        assert!(state.submit_framing(
            1,
            json!({"action":"framing","config":{"mode":"fixed","bytes":0}}),
            "fixed".into(),
            "0".into()
        ));
        state.finish_result(Lane::Session(1), Err(anyhow::anyhow!("invalid frame size")));
        assert_eq!(state.preferences.get("frame_mode"), before);
        assert_eq!(state.sessions[&1].preferences.get("frame_mode"), before);
        assert!(state.pending_framing.is_empty());
    }
}
