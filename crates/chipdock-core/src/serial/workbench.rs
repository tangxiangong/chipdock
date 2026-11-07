//! Validated workbench actions shared by the two native clients.
use anyhow::{Result, anyhow, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::{select, time::Instant};

use super::{
    SerialManager, bytes_to_hex, calculate_range, capture, encode_input,
    framing::{Framer, Framing},
    modbus, plot,
    program::{Program, Rule},
    types::{ChecksumAlgorithm, SendFormat, SendOptions},
    workflow,
};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Request {
    Plot {
        config: plot::Settings,
        #[serde(default)]
        cursor: u64,
        sample: Option<String>,
        #[serde(default)]
        hex: bool,
    },
    ValidatePackets {
        packets: Vec<workflow::Packet>,
    },
    StartJob {
        job: workflow::Job,
    },
    CancelJob,
    ExportMacro {
        path: std::path::PathBuf,
    },
    Replies {
        rules: Vec<workflow::ReplyRule>,
    },
    Capture {
        raw: bool,
        settings: Option<capture::Settings>,
    },
    Export {
        path: std::path::PathBuf,
        format: capture::Format,
        #[serde(default)]
        after: u64,
    },
    ResetCounters,
    Protocol {
        rules: Vec<Rule>,
    },
    Command {
        input: String,
    },
    Framing {
        config: Framing,
    },
    Checksum {
        input: String,
        format: SendFormat,
        algorithm: ChecksumAlgorithm,
        #[serde(default)]
        options: SendOptions,
    },
    Modbus {
        request: modbus::Request,
        #[serde(default)]
        timeout_ms: u64,
    },
}

pub async fn execute(session: &Arc<SerialManager>, input: &str) -> Result<String> {
    ensure!(input.len() <= 1024 * 1024, "工作台请求过大");
    let value: Value = match serde_json::from_str(input)? {
        Request::Plot {
            config,
            cursor,
            sample,
            hex,
        } => {
            config.validate()?;
            if let Some(sample) = sample {
                let bytes = if hex {
                    super::parse_hex(&sample)?
                } else {
                    sample.into_bytes()
                };
                json!({"values": config.parse(&bytes)})
            } else {
                let page = session.records(cursor, 96).await;
                let points: Vec<_> = page
                    .frames
                    .iter()
                    .flat_map(|frame| config.frame(frame))
                    .collect();
                json!({"points": points, "nextCursor": page.next_cursor, "droppedFrames": page.dropped_frames})
            }
        }
        Request::ValidatePackets { packets } => {
            workflow::validate_packets(&packets)?;
            json!({})
        }
        Request::StartJob { job } => {
            workflow::start(session, job).await?;
            json!({})
        }
        Request::CancelJob => {
            session.state.lock().await.job.cancel();
            json!({})
        }
        Request::ExportMacro { path } => {
            json!({"steps": workflow::export_report(session, &path).await?})
        }
        Request::Replies { rules } => {
            let mut responder = workflow::Responder::new(&rules)?;
            let mut state = session.state.lock().await;
            responder.revision = state.responder.revision.wrapping_add(1);
            state.responder = responder;
            json!({})
        }
        Request::Capture { raw, mut settings } => {
            let _operation = session.lifecycle.lock().await;
            let mut slot = {
                let mut state = session.state.lock().await;
                ensure!(!state.closed, "串口会话已关闭");
                let slot = if raw { &mut state.raw } else { &mut state.log };
                ensure!(
                    settings.is_none() || !slot.active(),
                    "请先停止正在写入的采集"
                );
                std::mem::take(slot)
            };
            let result: Result<()> = async {
                slot.stop().await?;
                if let Some(mut settings) = settings.take() {
                    if raw {
                        settings.format = capture::Format::Bin;
                    }
                    slot = tokio::task::spawn_blocking(move || capture::Capture::start(settings))
                        .await??;
                }
                Ok(())
            }
            .await;
            let active = slot.active();
            let mut state = session.state.lock().await;
            if raw {
                state.raw = slot;
            } else {
                state.log = slot;
            }
            result?;
            json!({"active": active})
        }
        Request::Export {
            path,
            format,
            after,
        } => {
            let records = session.records(after, 2000).await.frames;
            let count = records.len();
            tokio::task::spawn_blocking(move || capture::export(&path, format, records)).await??;
            json!({"records": count})
        }
        Request::ResetCounters => {
            let mut state = session.state.lock().await;
            state.tx_bytes = 0;
            state.rx_bytes = 0;
            state.count_start = state.started.map(|_| Instant::now());
            json!({})
        }
        Request::Protocol { rules } => {
            let program = Program::new(&rules)?;
            let mut state = session.state.lock().await;
            if state
                .selected_protocol
                .as_ref()
                .is_some_and(|name| !program.command_names().contains(name))
            {
                state.selected_protocol = None;
            }
            state.program = program;
            state.program_buffer.clear();
            state.parsed.clear();
            state.pending = None;
            state.feedback = None;
            json!({})
        }
        Request::Command { input } => json!({"hex": session.send_command(&input, false).await?}),
        Request::Framing { config } => {
            let framing = Framer::new(config)?;
            let mut state = session.state.lock().await;
            state.flush_rx();
            state.framer = framing;
            json!({})
        }
        Request::Checksum {
            input,
            format,
            algorithm,
            options,
        } => {
            json!({"hex": bytes_to_hex(&calculate_range(&encode_input(&input, format)?, algorithm, options)?)})
        }
        Request::Modbus {
            request,
            timeout_ms,
        } => {
            let bytes = request.encode()?;
            if timeout_ms == 0 {
                json!({"hex": bytes_to_hex(&bytes)})
            } else {
                ensure!(
                    (10..=60_000).contains(&timeout_ms),
                    "Modbus 超时须为 10–60000 毫秒"
                );
                let _tx = session.try_send()?;
                let (mut rx, mut cancel) = {
                    let state = session.state.lock().await;
                    let connection = state
                        .connection
                        .as_ref()
                        .ok_or_else(|| anyhow!("串口未连接"))?;
                    (connection.rx.subscribe(), connection.cancel.subscribe())
                };
                session.write(bytes.clone(), "modbus").await?;
                let reply = select! {
                    biased;
                    _ = cancel.wait_for(|ended| *ended) => return Err(anyhow!("串口连接已结束")),
                    result = tokio::time::timeout(Duration::from_millis(timeout_ms), async {
                        let mut buffer = Vec::new();
                        loop {
                            let chunk = rx.recv().await.map_err(|error| anyhow!("Modbus 接收中断：{error}"))?;
                            buffer.extend_from_slice(&chunk);
                            if let Some(reply) = request.receive(&bytes, &mut buffer) { return Ok::<_, anyhow::Error>(reply) }
                        }
                    }) => result.map_err(|_| anyhow!("等待有效 Modbus 响应超时"))??,
                };
                serde_json::to_value(reply)?
            }
        }
    };
    Ok(serde_json::to_string(&value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn capture_stop_reports_rotation_failure_and_allows_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.bin");
        std::fs::write(directory.path().join("capture.bin.0001"), b"existing").unwrap();
        let session = Arc::new(SerialManager::new());
        let start = json!({"action": "capture", "raw": true, "settings": {
            "path": path, "format": "bin", "rollBytes": 1
        }});
        execute(&session, &start.to_string()).await.unwrap();
        {
            let mut state = session.state.lock().await;
            state.receive(b"a");
            state.receive(b"b");
        }
        let stop = r#"{"action":"capture","raw":true}"#;
        let error = execute(&session, stop).await.unwrap_err().to_string();
        assert!(error.contains("采集写入失败"));
        let status = session.status().await;
        assert!(!status.capturing);
        assert_eq!(status.capture_error.as_deref(), Some(error.as_str()));
        assert_eq!(std::fs::read(&path).unwrap(), b"a");
        assert_eq!(
            std::fs::read(directory.path().join("capture.bin.0001")).unwrap(),
            b"existing"
        );

        let mut restart = start;
        restart["settings"]["path"] = json!(directory.path().join("restarted.bin"));
        execute(&session, &restart.to_string()).await.unwrap();
        assert!(session.status().await.capturing);
        assert!(session.status().await.capture_error.is_none());
        execute(&session, stop).await.unwrap();
    }
}
