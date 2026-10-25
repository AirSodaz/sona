//! TranslationEnginePort adapter implementation for ONNX NMT.

use crate::model::OnnxNmtModel;
use async_trait::async_trait;
use parking_lot::RwLock;
use sona_core::ports::translation::{
    TranslationEngineKind, TranslationEnginePort, TranslationPortError,
};
use std::path::PathBuf;
use std::sync::Arc;

pub struct OnnxNmtAdapter {
    model: Arc<RwLock<Option<OnnxNmtModel>>>,
    model_dir: Option<PathBuf>,
    num_threads: i32,
}

impl OnnxNmtAdapter {
    pub fn new(model_dir: Option<PathBuf>, num_threads: i32) -> Self {
        Self {
            model: Arc::new(RwLock::new(None)),
            model_dir,
            num_threads,
        }
    }

    pub fn with_model(model: OnnxNmtModel) -> Self {
        Self {
            model: Arc::new(RwLock::new(Some(model))),
            model_dir: None,
            num_threads: 2,
        }
    }

    pub fn ensure_loaded(&self) -> Result<(), TranslationPortError> {
        let read = self.model.read();
        if read.is_some() {
            return Ok(());
        }
        drop(read);

        let Some(dir) = &self.model_dir else {
            return Err(TranslationPortError::model_not_ready(
                "No model directory configured",
            ));
        };

        let loaded = OnnxNmtModel::load(dir, self.num_threads)?;
        let mut write = self.model.write();
        *write = Some(loaded);
        Ok(())
    }
}

#[async_trait]
impl TranslationEnginePort for OnnxNmtAdapter {
    async fn translate(
        &self,
        text: &str,
        _source_lang: &str,
        _target_lang: &str,
    ) -> Result<String, TranslationPortError> {
        self.ensure_loaded()?;
        let read = self.model.read();
        let model = read
            .as_ref()
            .ok_or_else(|| TranslationPortError::model_not_ready("ONNX NMT model is not loaded"))?;
        model.translate(text)
    }

    fn engine_kind(&self) -> TranslationEngineKind {
        TranslationEngineKind::OnnxNmt
    }
}
