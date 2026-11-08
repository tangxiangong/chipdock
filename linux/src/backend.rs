use crate::model::{Preferences, hex_dump, optional};
use anyhow::{Context, Result};
use chipdock_core::{models::*, serial::types::*};
use chipdock_service::{api::ChipDockCore, serial as service};
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool, mpsc};

pub enum Command {
    Scan,
    RepairProbe(String),
    Connect(Preferences),
    Preview(Preferences),
    Layout(Preferences),
    Search(String),
    Flash(Preferences),
    Erase(Preferences),
    Memory(Preferences),
    CheckUpdate,
    DownloadUpdate(crate::update::Available),
    InstallUpdate(std::path::PathBuf, crate::update::Available),
    Ports,
    NewSession(Preferences),
    InitializeSession(Preferences),
    CloseSession(u64),
    ConnectSerial(u64, Preferences),
    Disconnect(u64),
    Send(u64, Preferences),
    Line(u64, ControlLine, bool),
    Workbench(u64, Value),
    Plot(u64, u64, Value),
    Export(String, Vec<u8>),
    Import(String, String),
    Protocol(Value),
    Ipc(u8),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lane {
    Probe,
    Serial,
    Session(u64),
    Plot(u64),
    Files,
    Discovery,
    Update,
}
impl Lane {
    pub fn background(self) -> bool {
        matches!(self, Self::Plot(_) | Self::Discovery)
    }
}
impl Command {
    pub fn lane(&self) -> Lane {
        match self {
            Self::CheckUpdate | Self::DownloadUpdate(_) | Self::InstallUpdate(..) => Lane::Update,
            Self::Plot(id, ..) => Lane::Plot(*id),
            Self::CloseSession(id)
            | Self::ConnectSerial(id, _)
            | Self::Disconnect(id)
            | Self::Send(id, _)
            | Self::Line(id, ..)
            | Self::Workbench(id, _) => Lane::Session(*id),
            Self::Import(..) | Self::Protocol(_) | Self::Export(..) => Lane::Files,
            Self::Ports | Self::NewSession(_) | Self::InitializeSession(_) | Self::Ipc(_) => {
                Lane::Serial
            }
            _ => Lane::Probe,
        }
    }
}
pub enum Output {
    Update(Option<crate::update::Available>),
    Downloaded(std::path::PathBuf),
    Restart,
    Text(String),
    Sent(u64, String),
    CommandSent(u64, String, Value),
    Ipc(u8),
    Imported(String, String),
    Probes(Vec<ProbeSummary>),
    Chips(Vec<String>),
    Preview(FirmwarePreview, FirmwareInput),
    Memory(MemoryReadResult),
    Layout(Vec<MemoryRegionLayout>),
    Ports(Vec<PortInfo>),
    Session(u64, Option<String>),
    Closed(u64),
    Connected(ConnectionInfo),
    Serial(u64, SerialStatus),
    Workbench(u64, Value),
    Plot(u64, u64, Value),
}
pub struct Worker {
    pub sender: mpsc::Sender<Command>,
    pub receiver: mpsc::Receiver<(Lane, Result<Output>)>,
    pub core: Arc<ChipDockCore>,
    pub cancel_update: Arc<AtomicBool>,
    pub update_progress: mpsc::Receiver<(u64, u64)>,
}
impl Worker {
    pub fn new() -> Self {
        let core = ChipDockCore::new();
        let (sender, commands) = mpsc::channel::<Command>();
        let (results, receiver) = mpsc::channel();
        let cancel_update = Arc::new(AtomicBool::new(false));
        let (progress, update_progress) = mpsc::channel();
        let worker_core = core.clone();
        let worker_cancel = cancel_update.clone();
        std::thread::spawn(move || {
            let mut lanes = std::collections::HashMap::<Lane, mpsc::Sender<Command>>::new();
            while let Ok(command) = commands.recv() {
                let lane = command.lane();
                let closing = matches!(&command, Command::CloseSession(_));
                let sender = lanes.entry(lane).or_insert_with(|| {
                    let (tx, rx) = mpsc::channel();
                    let core = worker_core.clone();
                    let cancel = worker_cancel.clone();
                    let results = results.clone();
                    let progress = progress.clone();
                    std::thread::spawn(move || {
                        while let Ok(command) = rx.recv() {
                            if results
                                .send((lane, execute(&core, command, &cancel, &progress)))
                                .is_err()
                            {
                                break;
                            }
                        }
                    });
                    tx
                });
                if sender.send(command).is_err() {
                    break;
                }
                if closing {
                    if let Lane::Session(id) = lane {
                        lanes.remove(&Lane::Plot(id));
                    }
                    lanes.remove(&lane);
                }
            }
        });
        Self {
            sender,
            receiver,
            core,
            cancel_update,
            update_progress,
        }
    }
}
fn parse<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
    Ok(serde_json::from_value(json!(s))?)
}
pub fn serial_config(p: &Preferences) -> Result<SerialConfig> {
    let c = SerialConfig {
        baud_rate: p.number("baud")?.try_into()?,
        data_bits: p.number("bits")?.try_into()?,
        stop_bits: p.number("stop")?.try_into()?,
        parity: parse(p.get("parity"))?,
        flow_control: parse(p.get("flow"))?,
    };
    c.validate()?;
    Ok(c)
}
pub fn send_options(p: &Preferences) -> Result<SendOptions> {
    Ok(SendOptions {
        hex_line_ending: p.flag("hex_ending"),
        skip_prefix: p.number("prefix")?.try_into()?,
        skip_suffix: p.number("suffix")?.try_into()?,
    })
}
fn execute(
    core: &ChipDockCore,
    command: Command,
    cancel: &AtomicBool,
    progress: &mpsc::Sender<(u64, u64)>,
) -> Result<Output> {
    Ok(match command {
        Command::CheckUpdate => Output::Update(crate::update::check()?),
        Command::DownloadUpdate(update) => {
            Output::Downloaded(crate::update::download(&update, cancel, |done, total| {
                let _ = progress.send((done, total));
            })?)
        }
        Command::InstallUpdate(path, update) => {
            anyhow::ensure!(
                !service::serial_is_busy()?,
                "请先断开全部串口并停止任务，再安装更新"
            );
            crate::update::install(&path, &update)?;
            Output::Restart
        }
        Command::Import(key, path) => {
            use std::io::Read;
            let mut bytes = String::new();
            std::fs::File::open(path)?
                .take(1024 * 1024 + 1)
                .read_to_string(&mut bytes)?;
            anyhow::ensure!(bytes.len() <= 1024 * 1024, "导入文件超过 1 MiB");
            let _: Value = serde_json::from_str(&bytes)?;
            Output::Imported(key, bytes)
        }
        Command::Protocol(value) => {
            Output::Text(chipdock_core::serial::program::execute(&value.to_string())?)
        }
        Command::Plot(id, revision, value) => Output::Plot(
            id,
            revision,
            serde_json::from_str(&service::serial_workbench(id, &value.to_string())?)?,
        ),
        Command::Scan => Output::Probes(crate::platform::list_probes(core)?),
        Command::RepairProbe(selector) => Output::Text(crate::platform::repair_probe(&selector)?),
        Command::Search(query) => Output::Chips(core.search_chips(query, 100)?),
        Command::Connect(p) => Output::Connected(core.connect_target(ConnectRequest {
            probe: optional(p.get("probe")),
            target: p.target()?,
        })?),
        Command::Preview(p) => {
            let input = p.firmware()?;
            Output::Preview(core.firmware_preview(input.clone(), 4096)?, input)
        }
        Command::Layout(p) => Output::Layout(core.target_memory_map(p.get("chip").into())?),
        Command::Flash(p) => {
            core.flash_firmware(FlashRequest {
                firmware: p.firmware()?,
                probe: optional(p.get("probe")),
                target: p.target()?,
                options: p.options(),
            })?;
            Output::Text(
                if p.flag("dry_run") {
                    "预检查完成"
                } else {
                    "烧录完成"
                }
                .into(),
            )
        }
        Command::Erase(p) => {
            core.erase_target(EraseRequest {
                probe: optional(p.get("probe")),
                target: p.target()?,
            })?;
            Output::Text("擦除完成".into())
        }
        Command::Memory(p) => Output::Memory(core.read_memory(MemoryRequest {
            probe: optional(p.get("probe")),
            target: p.target()?,
            address: p.number("address")?,
            length: p.number("length")?.try_into()?,
        })?),
        Command::Ports => Output::Ports(service::serial_ports()?),
        Command::NewSession(p) => {
            let id = service::serial_create_session()?;
            let warning = configure_session(id, &p).err().map(|e| e.to_string());
            Output::Session(id, warning)
        }
        Command::InitializeSession(p) => {
            let id = service::DEFAULT_SESSION;
            let warning = configure_session(id, &p).err().map(|e| e.to_string());
            Output::Session(id, warning)
        }
        Command::CloseSession(id) => {
            service::serial_close_session(id)?;
            Output::Closed(id)
        }
        Command::ConnectSerial(id, p) => {
            configure_session(id, &p)?;
            Output::Serial(
                id,
                service::serial_session_connect(
                    id,
                    p.get("port").into(),
                    serial_config(&p)?,
                    p.flag("reconnect"),
                )?,
            )
        }
        Command::Disconnect(id) => Output::Serial(id, service::serial_session_disconnect(id)?),
        Command::Line(id, line, value) => {
            Output::Serial(id, service::serial_session_set_line(id, line, value)?)
        }
        Command::Send(id, p) => {
            service::serial_session_send_options(
                id,
                p.get("send").into(),
                parse(p.get("format"))?,
                parse(p.get("ending"))?,
                parse(p.get("checksum"))?,
                send_options(&p)?,
            )?;
            Output::Sent(id, p.get("send").into())
        }
        Command::Workbench(id, value) => {
            let result = serde_json::from_str(&service::serial_workbench(id, &value.to_string())?)?;
            if value["action"] == "command" {
                Output::CommandSent(id, value["input"].as_str().unwrap_or("").into(), result)
            } else {
                Output::Workbench(id, result)
            }
        }
        Command::Export(path, bytes) => {
            crate::model::write_atomic(std::path::Path::new(&path), &bytes)
                .with_context(|| format!("无法导出到 {path}"))?;
            Output::Text(format!("已导出到 {path}"))
        }
        Command::Ipc(mode) => {
            service::ipc::configure(mode)?;
            Output::Ipc(mode)
        }
    })
}
pub fn memory_text(memory: &MemoryReadResult) -> Result<String> {
    let bytes = crate::model::decode_hex(&memory.data_hex)?;
    Ok(hex_dump(
        memory.address,
        &bytes.into_iter().map(Some).collect::<Vec<_>>(),
    ))
}

impl Default for Worker {
    fn default() -> Self {
        Self::new()
    }
}

fn configure_session(id: u64, p: &Preferences) -> Result<()> {
    for action in ["protocol", "replies", "framing"] {
        service::serial_workbench(id, &crate::serial::request(p, action)?.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_session_is_initialized_without_a_phantom_session() {
        let (tx, _) = mpsc::channel();
        let result = execute(
            &ChipDockCore::default(),
            Command::InitializeSession(Preferences::default()),
            &AtomicBool::new(false),
            &tx,
        )
        .unwrap();
        assert!(matches!(
            result,
            Output::Session(service::DEFAULT_SESSION, None)
        ));
    }
    #[test]
    fn serial_sessions_and_plot_have_independent_dispatch_lanes() {
        assert_ne!(
            Command::Send(1, Preferences::default()).lane(),
            Command::Send(2, Preferences::default()).lane()
        );
        assert_ne!(
            Command::Plot(1, 0, json!({})).lane(),
            Command::Send(1, Preferences::default()).lane()
        );
    }
}

#[cfg(test)]
mod concurrency_tests {
    use super::*;
    #[test]
    fn waiting_for_modbus_on_one_virtual_session_does_not_block_another_session() {
        let a = service::serial_create_session().unwrap();
        let b = service::serial_create_session().unwrap();
        service::serial_session_connect(a, "VIRTUAL-A".into(), SerialConfig::default(), false)
            .unwrap();
        service::serial_session_connect(b, "VIRTUAL-B".into(), SerialConfig::default(), false)
            .unwrap();
        let worker = Worker::new();
        worker.sender.send(Command::Workbench(a,json!({"action":"modbus","request":{"station":1,"function":3,"address":0,"quantity":1,"values":[]},"timeout_ms":2000}))).unwrap();
        worker
            .sender
            .send(Command::Workbench(
                b,
                json!({"action":"checksum","input":"01 02 FF","format":"hex","algorithm":"sum8"}),
            ))
            .unwrap();
        let (lane, output) = worker
            .receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        assert_eq!(lane, Lane::Session(b));
        assert!(matches!(output.unwrap(),Output::Workbench(id,_) if id==b));
        let (lane, result) = worker
            .receiver
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        assert_eq!(lane, Lane::Session(a));
        assert!(result.err().unwrap().to_string().contains("超时"));
        let received = service::serial_session_records(b, 0, 256).unwrap();
        assert!(
            received
                .frames
                .iter()
                .any(|f| f.bytes == [1, 3, 0, 0, 0, 1, 0x84, 0x0a])
        );
        service::serial_close_session(a).unwrap();
        service::serial_close_session(b).unwrap();
    }
}
