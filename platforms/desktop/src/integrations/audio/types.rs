use std::sync::Arc;

pub const MICROPHONE_PEAK_EVENT: &str = "microphone-audio";
pub const SYSTEM_PEAK_EVENT: &str = "system-audio";
pub const AUDIO_CAPTURE_ERROR_EVENT: &str = "audio-capture-error";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CaptureKind {
    System,
    Microphone,
}

impl CaptureKind {
    pub fn label(self) -> &'static str {
        match self {
            CaptureKind::System => "System",
            CaptureKind::Microphone => "Microphone",
        }
    }

    pub fn log_name(self) -> &'static str {
        match self {
            CaptureKind::System => "system",
            CaptureKind::Microphone => "microphone",
        }
    }

    pub fn peak_event(self) -> &'static str {
        match self {
            CaptureKind::System => SYSTEM_PEAK_EVENT,
            CaptureKind::Microphone => MICROPHONE_PEAK_EVENT,
        }
    }

    pub fn stop_log_label(self) -> &'static str {
        match self {
            CaptureKind::System => "System",
            CaptureKind::Microphone => "Mic",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CaptureKey {
    pub kind: CaptureKind,
    pub device_name: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveCaptureLease {
    pub source_id: String,
    pub source_generation: u64,
    pub source_cursor: u64,
}

#[derive(Clone)]
pub struct AudioStopSignal {
    stopper: Arc<dyn Fn() + Send + Sync>,
    joiner: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl std::fmt::Debug for AudioStopSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioStopSignal")
            .field("has_joiner", &self.joiner.is_some())
            .finish()
    }
}

impl AudioStopSignal {
    pub fn new<F: Fn() + Send + Sync + 'static>(f: F) -> Self {
        Self {
            stopper: Arc::new(f),
            joiner: None,
        }
    }

    pub fn with_joiner<F, J>(stop: F, join: J) -> Self
    where
        F: Fn() + Send + Sync + 'static,
        J: Fn() + Send + Sync + 'static,
    {
        Self {
            stopper: Arc::new(stop),
            joiner: Some(Arc::new(join)),
        }
    }

    pub fn from_sender(sender: std::sync::mpsc::Sender<()>) -> Self {
        Self::new(move || {
            let _ = sender.send(());
        })
    }

    /// Signals the hardware capture stream to stop non-blockingly.
    pub fn stop(&self) {
        (self.stopper)();
    }

    /// Waits for the underlying hardware stream/thread to join.
    /// MUST be called within a blocking context (e.g. `spawn_blocking`),
    /// NEVER directly on a Tokio async worker thread.
    pub fn join_hardware(&self) {
        if let Some(joiner) = &self.joiner {
            (joiner)();
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AudioCaptureError {
    #[error("Unsupported native capture source: {0}")]
    UnsupportedSource(String),
    #[error("Capture instance '{0}' is not active")]
    InstanceNotActive(String),
    #[error("Capture registry entry is missing")]
    MissingRegistryEntry,
    #[error("Failed to update pause state: {0}")]
    PauseState(String),
    #[error("Poisoned lock: {0}")]
    LockPoisoned(String),
    #[error("{0}")]
    Other(String),
}
