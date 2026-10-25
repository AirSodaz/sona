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
        {
            let read = self.model.read();
            if read.is_some() {
                return Ok(());
            }
        }

        let Some(dir) = &self.model_dir else {
            return Err(TranslationPortError::model_not_ready(
                "No model directory configured",
            ));
        };

        let mut write = self.model.write();
        if write.is_some() {
            return Ok(());
        }

        let loaded = OnnxNmtModel::load(dir, self.num_threads)?;
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
        let text = text.to_string();
        let model_arc = Arc::clone(&self.model);
        let model_dir = self.model_dir.clone();
        let num_threads = self.num_threads;

        tokio::task::spawn_blocking(move || {
            {
                let read = model_arc.read();
                if read.is_none() {
                    drop(read);
                    let Some(dir) = &model_dir else {
                        return Err(TranslationPortError::model_not_ready(
                            "No model directory configured",
                        ));
                    };
                    let mut write = model_arc.write();
                    if write.is_none() {
                        let loaded = OnnxNmtModel::load(dir, num_threads)?;
                        *write = Some(loaded);
                    }
                }
            }
            let read = model_arc.read();
            let model = read.as_ref().ok_or_else(|| {
                TranslationPortError::model_not_ready("ONNX NMT model is not loaded")
            })?;
            model.translate(&text)
        })
        .await
        .map_err(|e| TranslationPortError::engine_failed(e.to_string()))?
    }

    fn engine_kind(&self) -> TranslationEngineKind {
        TranslationEngineKind::OnnxNmt
    }
}
