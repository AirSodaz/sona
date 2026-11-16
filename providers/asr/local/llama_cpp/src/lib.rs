pub mod adapter;
pub mod batch;
pub mod llm;
pub mod streaming;
pub mod translation;

/// Decodes one token into `output`, replacing its previous contents.
///
/// `decode_to_string` writes only into spare capacity and never grows the
/// string, so the bound must be reserved first. The bound also accounts for a
/// partial UTF-8 sequence carried over from the previous token.
pub(crate) fn decode_piece_into(
    decoder: &mut encoding_rs::Decoder,
    bytes: &[u8],
    output: &mut String,
) {
    output.clear();
    output.reserve(
        decoder
            .max_utf8_buffer_length(bytes.len())
            .expect("token output is too large to decode"),
    );
    let (result, read, _) = decoder.decode_to_string(bytes, output, false);
    assert!(
        matches!(result, encoding_rs::CoderResult::InputEmpty) && read == bytes.len(),
        "UTF-8 decoder capacity bound must consume the complete token"
    );
}

pub use adapter::LlamaCppAdapter;
pub use batch::{clear_all_llama_models, prune_idle_llama_models};
pub use llama_cpp_2;
pub use llm::{
    LlamaCppLlmEngine, LlamaModelHandle, LlamaModelProvider, clear_all_llm_models,
    prune_idle_llm_models,
};
pub use streaming::{LlamaCppStreamingFactory, Qwen3PseudoStreamDecoder};
pub use translation::LlamaCppTranslationAdapter;

#[cfg(test)]
mod tests {
    use super::decode_piece_into;

    #[test]
    fn decode_piece_handles_split_utf8_tokens() {
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut piece = String::new();
        decode_piece_into(&mut decoder, &[0xE4, 0xB8], &mut piece);
        assert_eq!(piece, "");
        decode_piece_into(&mut decoder, &[0xAD], &mut piece);
        assert_eq!(piece, "中");
        // Each call replaces the previous piece instead of appending to it.
        decode_piece_into(&mut decoder, b"ok", &mut piece);
        assert_eq!(piece, "ok");
    }
}
