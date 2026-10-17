use crate::{
    error::Result,
    models::{JobEvent, JobId, JobKind, JobStage},
};
use chrono::Utc;
use probe_rs::flashing::{FlashProgress, ProgressEvent as ProbeEvent, ProgressOperation};
use std::{collections::VecDeque, sync::Mutex};
use uuid::Uuid;

/// Bounded progress queue, independent of the UI runtime.
#[derive(Default)]
pub struct JobEvents(Mutex<VecDeque<JobEvent>>);

impl JobEvents {
    /// Drain events without acquiring the hardware-operation lock.
    pub fn drain(&self) -> Vec<JobEvent> {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .drain(..)
            .collect()
    }

    fn push(&self, event: JobEvent) {
        let mut events = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if events.len() >= 512 {
            events.pop_front();
        }
        events.push_back(event);
    }
}

/// 生成新的后台任务 ID。
pub fn new_job_id() -> JobId {
    JobId(Uuid::new_v4())
}

/// 向前端广播后台任务进度事件。
pub fn emit_job_event(
    app: &JobEvents,
    id: &JobId,
    kind: JobKind,
    stage: JobStage,
    progress: Option<f32>,
    message: impl Into<String>,
) -> Result<()> {
    let event = JobEvent {
        id: id.clone(),
        kind,
        stage,
        progress,
        message: message.into(),
        at: Utc::now(),
    };

    app.push(event);
    Ok(())
}

#[derive(Clone, Copy, Default)]
struct OperationProgress {
    total: Option<u64>,
    has_total: bool,
    done: u64,
    last_percent: Option<u8>,
}

struct ProbeProgressState<'a> {
    events: &'a JobEvents,
    id: &'a JobId,
    kind: JobKind,
    operations: [OperationProgress; 4],
    current: ProgressOperation,
}

impl ProbeProgressState<'_> {
    fn handle(&mut self, event: ProbeEvent) {
        match event {
            ProbeEvent::AddProgressBar { operation, total } => {
                let counter = &mut self.operations[operation_details(operation).0];
                counter.total = if counter.has_total {
                    counter
                        .total
                        .zip(total)
                        .and_then(|(left, right)| left.checked_add(right))
                } else {
                    total
                };
                counter.has_total = true;
            }
            ProbeEvent::Started(operation) => {
                self.current = operation;
                self.report(operation, true);
            }
            ProbeEvent::Progress {
                operation, size, ..
            } => {
                let counter = &mut self.operations[operation_details(operation).0];
                counter.done = counter.done.saturating_add(size);
                self.report(operation, false);
            }
            ProbeEvent::Finished(operation) => self.report(operation, true),
            ProbeEvent::DiagnosticMessage { message } => {
                let (_, stage, _) = operation_details(self.current);
                let _ = emit_job_event(
                    self.events,
                    self.id,
                    self.kind.clone(),
                    stage,
                    None,
                    message,
                );
            }
            // The caller emits one terminal event with the actual error chain.
            ProbeEvent::Failed(_) | ProbeEvent::FlashLayoutReady { .. } => {}
        }
    }

    fn report(&mut self, operation: ProgressOperation, force: bool) {
        let (index, stage, message) = operation_details(operation);
        let counter = &mut self.operations[index];
        let progress = counter
            .total
            .filter(|total| *total > 0)
            .map(|total| (counter.done as f64 / total as f64).min(1.0) as f32);
        let percent = progress.map(|value| (value * 100.0).round() as u8);
        // Page callbacks can be very frequent. Keep meaningful 1% changes,
        // preserving stage boundaries without flooding logs or the event queue.
        if !force && (percent.is_none() || percent == counter.last_percent) {
            return;
        }
        counter.last_percent = percent;
        let message = match percent {
            Some(percent) => format!("{message}（{percent}%）"),
            None => message.to_string(),
        };
        let _ = emit_job_event(
            self.events,
            self.id,
            self.kind.clone(),
            stage,
            progress,
            message,
        );
    }
}

fn operation_details(operation: ProgressOperation) -> (usize, JobStage, &'static str) {
    match operation {
        ProgressOperation::Fill => (0, JobStage::Preparing, "正在准备写入数据"),
        ProgressOperation::Erase => (1, JobStage::Erasing, "正在擦除 Flash"),
        ProgressOperation::Program => (2, JobStage::Programming, "正在写入固件"),
        ProgressOperation::Verify => (3, JobStage::Verifying, "正在校验固件"),
    }
}

/// Forward real probe-rs stage/byte progress. Unknown-duration chip erases stay
/// indeterminate; successful completion is emitted only after the operation returns.
pub fn probe_progress<'a>(
    events: &'a JobEvents,
    id: &'a JobId,
    kind: JobKind,
) -> FlashProgress<'a> {
    let mut state = ProbeProgressState {
        events,
        id,
        kind,
        operations: [OperationProgress::default(); 4],
        current: ProgressOperation::Fill,
    };
    FlashProgress::new(move |event| state.handle(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_should_keep_latest_events_and_drain_once() {
        let events = JobEvents::default();
        let id = new_job_id();
        for index in 0..600 {
            emit_job_event(
                &events,
                &id,
                JobKind::Flash,
                JobStage::Programming,
                None,
                index.to_string(),
            )
            .unwrap();
        }
        let drained = events.drain();
        assert_eq!(drained.len(), 512);
        assert_eq!(drained.last().unwrap().message, "599");
        assert!(events.drain().is_empty());
    }

    #[test]
    fn real_progress_should_include_erase_program_and_verify_stages() {
        let events = JobEvents::default();
        let id = new_job_id();
        let mut progress = probe_progress(&events, &id, JobKind::Flash);
        for operation in [
            ProgressOperation::Erase,
            ProgressOperation::Program,
            ProgressOperation::Verify,
        ] {
            progress.emit(ProbeEvent::AddProgressBar {
                operation,
                total: Some(100),
            });
            progress.emit(ProbeEvent::Started(operation));
            progress.emit(ProbeEvent::Progress {
                operation,
                size: 50,
                time: std::time::Duration::ZERO,
            });
            progress.emit(ProbeEvent::Finished(operation));
        }
        let drained = events.drain();
        assert!(
            drained.iter().any(
                |event| matches!(event.stage, JobStage::Erasing) && event.progress == Some(0.5)
            )
        );
        assert!(drained.iter().any(
            |event| matches!(event.stage, JobStage::Programming) && event.progress == Some(0.5)
        ));
        assert!(
            drained
                .iter()
                .any(|event| matches!(event.stage, JobStage::Verifying)
                    && event.progress == Some(0.5))
        );
        assert!(
            !drained
                .iter()
                .any(|event| matches!(event.stage, JobStage::Completed))
        );
    }

    #[test]
    fn multiple_flash_algorithms_should_accumulate_stage_totals() {
        let events = JobEvents::default();
        let id = new_job_id();
        let mut progress = probe_progress(&events, &id, JobKind::Flash);
        for _ in 0..2 {
            progress.emit(ProbeEvent::AddProgressBar {
                operation: ProgressOperation::Program,
                total: Some(100),
            });
        }
        progress.emit(ProbeEvent::Progress {
            operation: ProgressOperation::Program,
            size: 100,
            time: std::time::Duration::ZERO,
        });
        assert_eq!(events.drain().last().unwrap().progress, Some(0.5));
    }

    #[test]
    fn chip_erase_without_a_known_total_should_not_invent_a_percentage() {
        let events = JobEvents::default();
        let id = new_job_id();
        let mut progress = probe_progress(&events, &id, JobKind::Erase);
        progress.emit(ProbeEvent::AddProgressBar {
            operation: ProgressOperation::Erase,
            total: None,
        });
        progress.emit(ProbeEvent::Started(ProgressOperation::Erase));
        progress.emit(ProbeEvent::Progress {
            operation: ProgressOperation::Erase,
            size: 4096,
            time: std::time::Duration::ZERO,
        });
        assert!(events.drain().iter().all(|event| event.progress.is_none()));
    }

    #[test]
    fn page_callbacks_should_be_throttled_without_losing_final_byte_progress() {
        let events = JobEvents::default();
        let id = new_job_id();
        let mut progress = probe_progress(&events, &id, JobKind::Flash);
        progress.emit(ProbeEvent::AddProgressBar {
            operation: ProgressOperation::Program,
            total: Some(10_000),
        });
        for _ in 0..10_000 {
            progress.emit(ProbeEvent::Progress {
                operation: ProgressOperation::Program,
                size: 1,
                time: std::time::Duration::ZERO,
            });
        }
        progress.emit(ProbeEvent::Finished(ProgressOperation::Program));
        let drained = events.drain();
        assert!(drained.len() <= 102);
        assert_eq!(drained.last().unwrap().progress, Some(1.0));
    }
}
