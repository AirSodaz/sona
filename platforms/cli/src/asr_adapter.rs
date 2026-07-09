use sona_core::ports::asr::BatchTranscriber;

pub(crate) fn local_batch_transcriber() -> impl BatchTranscriber {
    sona_local_asr::batch::LocalBatchAsrAdapter
}
