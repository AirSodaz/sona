use criterion::{criterion_group, criterion_main, Criterion};

// We will simulate the performance of the loop in `llm.rs` where the string cloning occurs.

#[derive(Clone)]
struct LlmSegmentInput {
    id: String,
}

#[derive(Clone)]
struct TranslatedSegment {
    id: String,
    translation: String,
}

#[derive(Clone)]
struct GoogleTranslation {
    translated_text: String,
}

#[derive(Clone)]
struct GoogleTranslateData {
    translations: Vec<GoogleTranslation>,
}

#[derive(Clone)]
struct GoogleTranslateResponse {
    data: GoogleTranslateData,
}

fn bench_current_impl(c: &mut Criterion) {
    let mut group = c.benchmark_group("llm_translation_parse");
    for size in [10, 100, 1000].iter() {
        group.bench_with_input(criterion::BenchmarkId::new("current", size), size, |b, &size| {
            let chunk: Vec<LlmSegmentInput> = (0..size).map(|i| LlmSegmentInput { id: format!("segment_id_{}", i) }).collect();
            let mut parsed = GoogleTranslateResponse {
                data: GoogleTranslateData {
                    translations: (0..size).map(|_| GoogleTranslation { translated_text: "Translated text content that is somewhat long so it actually allocates".to_string() }).collect(),
                }
            };

            b.iter(|| {
                let parsed_clone = parsed.clone(); // We have to clone to have ownership as in the real code
                let mut translated_segments = Vec::with_capacity(chunk.len());
                for (index, translation) in parsed_clone.data.translations.into_iter().enumerate() {
                    translated_segments.push(TranslatedSegment {
                        id: chunk[index].id.clone(),
                        translation: translation.translated_text,
                    });
                }
                criterion::black_box(translated_segments);
            });
        });
    }
    group.finish();
}

fn bench_optimized_impl(c: &mut Criterion) {
    let mut group = c.benchmark_group("llm_translation_parse");
    for size in [10, 100, 1000].iter() {
        group.bench_with_input(criterion::BenchmarkId::new("optimized", size), size, |b, &size| {
            let chunk: Vec<LlmSegmentInput> = (0..size).map(|i| LlmSegmentInput { id: format!("segment_id_{}", i) }).collect();
            let mut parsed = GoogleTranslateResponse {
                data: GoogleTranslateData {
                    translations: (0..size).map(|_| GoogleTranslation { translated_text: "Translated text content that is somewhat long so it actually allocates".to_string() }).collect(),
                }
            };

            b.iter(|| {
                let parsed_clone = parsed.clone();
                let translated_segments: Vec<_> = chunk.iter().zip(parsed_clone.data.translations).map(|(s, t)| TranslatedSegment {
                    id: s.id.clone(),
                    translation: t.translated_text,
                }).collect();
                criterion::black_box(translated_segments);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_current_impl, bench_optimized_impl);
criterion_main!(benches);
