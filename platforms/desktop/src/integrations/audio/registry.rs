use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use sona_application::live_transcription::LiveSourceEpoch;

use super::recorder::RecorderCommand;
use super::types::{AudioStopSignal, CaptureKey, CaptureKind, LiveCaptureLease};

pub struct SharedCaptureDetachResult {
    pub should_stop_hardware: bool,
    pub was_recording: bool,
    pub stop_signal: Option<AudioStopSignal>,
    pub recorder_tx: Option<tokio::sync::mpsc::Sender<RecorderCommand>>,
}

#[derive(Default)]
pub struct SharedCaptureState {
    pub stop_signal: Option<AudioStopSignal>,
    pub instance_ids: HashSet<String>,
    pub paused_instances: HashSet<String>,
    pub recording_instances: HashSet<String>,
    pub recorder_tx: Option<tokio::sync::mpsc::Sender<RecorderCommand>>,
    pub source: LiveSourceEpoch,
    pub sample_cursor: Arc<AtomicU64>,
}

impl SharedCaptureState {
    pub fn is_running(&self) -> bool {
        self.stop_signal.is_some()
    }

    pub fn owners(&self) -> Vec<String> {
        let mut owners = self.instance_ids.iter().cloned().collect::<Vec<_>>();
        owners.sort();
        owners
    }

    pub fn active_instances(&self) -> Vec<String> {
        let mut active_instances = self
            .instance_ids
            .iter()
            .filter(|instance_id| !self.paused_instances.contains(*instance_id))
            .cloned()
            .collect::<Vec<_>>();
        active_instances.sort();
        active_instances
    }

    pub fn attach_instance(&mut self, instance_id: String, should_record: bool) -> Vec<String> {
        self.paused_instances.remove(&instance_id);
        self.instance_ids.insert(instance_id.clone());
        if should_record {
            self.recording_instances.insert(instance_id);
        } else {
            self.recording_instances.remove(&instance_id);
        }
        self.owners()
    }

    #[cfg(test)]
    pub fn commit_start(
        &mut self,
        instance_id: String,
        stop_signal: AudioStopSignal,
        recorder_tx: tokio::sync::mpsc::Sender<RecorderCommand>,
    ) -> Vec<String> {
        self.commit_start_with_source(
            instance_id,
            true,
            stop_signal,
            recorder_tx,
            LiveSourceEpoch::default(),
            Arc::new(AtomicU64::new(0)),
        )
    }

    pub fn commit_start_with_source(
        &mut self,
        instance_id: String,
        should_record: bool,
        stop_signal: AudioStopSignal,
        recorder_tx: tokio::sync::mpsc::Sender<RecorderCommand>,
        source: LiveSourceEpoch,
        sample_cursor: Arc<AtomicU64>,
    ) -> Vec<String> {
        self.instance_ids.clear();
        self.paused_instances.clear();
        self.recording_instances.clear();
        self.instance_ids.insert(instance_id.clone());
        if should_record {
            self.recording_instances.insert(instance_id);
        }
        self.stop_signal = Some(stop_signal);
        self.recorder_tx = Some(recorder_tx);
        self.source = source;
        self.sample_cursor = sample_cursor;
        self.owners()
    }

    pub fn set_instance_paused(
        &mut self,
        instance_id: &str,
        paused: bool,
    ) -> Result<Vec<String>, String> {
        if !self.instance_ids.contains(instance_id) {
            return Err(format!(
                "Capture instance '{instance_id}' is not attached to the active session"
            ));
        }

        if paused {
            self.paused_instances.insert(instance_id.to_string());
        } else {
            self.paused_instances.remove(instance_id);
        }

        Ok(self.active_instances())
    }

    pub fn detach_instance(&mut self, instance_id: &str) -> SharedCaptureDetachResult {
        self.instance_ids.remove(instance_id);
        self.paused_instances.remove(instance_id);
        let was_recording = self.recording_instances.remove(instance_id);
        let should_stop_hardware = self.instance_ids.is_empty();
        let recorder_tx = if should_stop_hardware {
            self.recorder_tx.take()
        } else {
            self.recorder_tx.clone()
        };
        let stop_signal = if should_stop_hardware {
            self.stop_signal.take()
        } else {
            None
        };
        SharedCaptureDetachResult {
            should_stop_hardware,
            was_recording,
            stop_signal,
            recorder_tx,
        }
    }

    pub fn lease(&self) -> LiveCaptureLease {
        LiveCaptureLease {
            source_id: self.source.source_id.clone(),
            source_generation: self.source.generation,
            source_cursor: self.sample_cursor.load(Ordering::Acquire),
        }
    }
}

#[derive(Default)]
pub struct CaptureRegistry {
    pub captures: HashMap<CaptureKey, SharedCaptureState>,
    pub instance_keys: HashMap<(CaptureKind, String), CaptureKey>,
    pub stopping_keys: HashMap<CaptureKey, (LiveSourceEpoch, tokio::sync::watch::Receiver<bool>)>,
    stopping_senders: HashMap<CaptureKey, (LiveSourceEpoch, tokio::sync::watch::Sender<bool>)>,
}

impl CaptureRegistry {
    pub fn is_stopping(&self, key: &CaptureKey) -> Option<tokio::sync::watch::Receiver<bool>> {
        self.stopping_keys.get(key).map(|(_, rx)| rx.clone())
    }

    pub fn mark_stopping(
        &mut self,
        key: CaptureKey,
        source: LiveSourceEpoch,
    ) -> tokio::sync::watch::Receiver<bool> {
        let (tx, rx) = tokio::sync::watch::channel(false);
        self.stopping_keys
            .insert(key.clone(), (source.clone(), rx.clone()));
        self.stopping_senders.insert(key, (source, tx));
        rx
    }

    pub fn clear_stopping(&mut self, key: &CaptureKey, source: &LiveSourceEpoch) -> bool {
        let matches = self
            .stopping_senders
            .get(key)
            .is_some_and(|(s, _)| s == source);
        if matches {
            self.stopping_keys.remove(key);
            if let Some((_, tx)) = self.stopping_senders.remove(key) {
                let _ = tx.send(true);
            }
            true
        } else {
            false
        }
    }

    pub fn attach_running(
        &mut self,
        key: &CaptureKey,
        kind: CaptureKind,
        instance_id: &str,
        should_record: bool,
    ) -> Result<Option<(LiveCaptureLease, tokio::sync::mpsc::Sender<RecorderCommand>)>, String>
    {
        let instance_key = (kind, instance_id.to_string());
        if let Some(existing_key) = self.instance_keys.get(&instance_key)
            && existing_key != key
        {
            return Err(format!(
                "Capture instance '{instance_id}' is already attached to another device"
            ));
        }
        let Some(capture) = self.captures.get_mut(key) else {
            return Ok(None);
        };
        if !capture.is_running() {
            return Ok(None);
        }
        let recorder_tx = capture
            .recorder_tx
            .clone()
            .ok_or_else(|| "Capture recorder task is unavailable".to_string())?;
        capture.attach_instance(instance_id.to_string(), should_record);
        let lease = capture.lease();
        self.instance_keys.insert(instance_key, key.clone());
        Ok(Some((lease, recorder_tx)))
    }

    pub fn rollback_attachment(
        &mut self,
        kind: CaptureKind,
        instance_id: &str,
        key: &CaptureKey,
    ) -> Option<AudioStopSignal> {
        let instance_key = (kind, instance_id.to_string());
        if self.instance_keys.get(&instance_key) != Some(key) {
            return None;
        }
        self.instance_keys.remove(&instance_key);
        let capture = self.captures.get_mut(key)?;
        let source = capture.source.clone();
        let detach = capture.detach_instance(instance_id);
        if detach.should_stop_hardware {
            self.captures.remove(key);
            self.clear_stopping(key, &source);
        }
        detach.stop_signal
    }

    pub fn has_active_captures(&self) -> bool {
        !self.captures.is_empty() || !self.stopping_keys.is_empty()
    }
}

pub fn update_capture_pause_state(
    capture: &mut SharedCaptureState,
    instance_id: &str,
    paused: bool,
    capture_label: &str,
    should_record: bool,
) -> Result<(), String> {
    if !capture.is_running() {
        return Err(format!("{} capture is not running", capture_label));
    }

    capture.set_instance_paused(instance_id, paused)?;

    if should_record {
        let recorder_tx = capture
            .recorder_tx
            .as_ref()
            .ok_or_else(|| format!("{} recorder task is not available", capture_label))?;

        recorder_tx
            .try_send(RecorderCommand::SetPaused {
                owner: instance_id.to_string(),
                paused,
            })
            .map_err(|err| {
                format!(
                    "Failed to {} {} recorder: {}",
                    if paused { "pause" } else { "resume" },
                    capture_label.to_lowercase(),
                    err
                )
            })?;
    }

    Ok(())
}
