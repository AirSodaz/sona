use regex::{NoExpand, Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;

use crate::transcription::TranscriptPostprocessError;
use crate::transcription::transcript::{TranscriptSegment, TranscriptUpdate};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTextReplacementRule {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTextReplacementRuleSet {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub ignore_case: bool,
    #[serde(default)]
    pub rules: Vec<TranscriptTextReplacementRule>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptPostprocessOptions {
    #[serde(default)]
    pub text_replacement_sets: Vec<TranscriptTextReplacementRuleSet>,
    #[serde(default = "default_drop_final_dot_segments")]
    pub drop_final_dot_segments: bool,
}

fn default_drop_final_dot_segments() -> bool {
    true
}

impl Default for TranscriptPostprocessOptions {
    fn default() -> Self {
        Self {
            text_replacement_sets: Vec::new(),
            drop_final_dot_segments: default_drop_final_dot_segments(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptNormalizationOptions {
    pub enable_timeline: bool,
}

#[derive(Clone, Debug)]
struct CompiledTextReplacementRule {
    regex: Regex,
    replacement: String,
}

#[derive(Clone, Debug)]
pub struct TranscriptPostprocessor {
    drop_final_dot_segments: bool,
    rules: Vec<CompiledTextReplacementRule>,
}

impl TranscriptPostprocessor {
    pub fn compile(
        options: TranscriptPostprocessOptions,
    ) -> Result<Self, TranscriptPostprocessError> {
        let rules = active_text_replacement_rules(&options.text_replacement_sets)
            .into_iter()
            .map(|rule| {
                let pattern = rule.from.clone();
                let regex = RegexBuilder::new(&regex::escape(&rule.from))
                    .case_insensitive(rule.ignore_case)
                    .build()
                    .map_err(|error| TranscriptPostprocessError::RuleCompilation {
                        pattern,
                        reason: error.to_string(),
                    })?;
                Ok(CompiledTextReplacementRule {
                    regex,
                    replacement: rule.to,
                })
            })
            .collect::<Result<Vec<_>, TranscriptPostprocessError>>()?;

        Ok(Self {
            drop_final_dot_segments: options.drop_final_dot_segments,
            rules,
        })
    }

    pub fn process_segments(&self, segments: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
        segments
            .into_iter()
            .filter_map(|mut segment| {
                if self.drop_final_dot_segments && segment.is_final && segment.text == "." {
                    return None;
                }
                segment.text = self.apply_text_replacements(&segment.text);
                Some(segment)
            })
            .collect()
    }

    pub fn process_update(&self, update: TranscriptUpdate) -> TranscriptUpdate {
        let mut remove_ids = update.remove_ids;
        let mut upsert_segments = Vec::with_capacity(update.upsert_segments.len());

        for mut segment in update.upsert_segments {
            if self.drop_final_dot_segments && segment.is_final && segment.text == "." {
                if !remove_ids.iter().any(|id| id == &segment.id) {
                    remove_ids.push(segment.id);
                }
                continue;
            }
            segment.text = self.apply_text_replacements(&segment.text);
            upsert_segments.push(segment);
        }

        TranscriptUpdate {
            remove_ids,
            upsert_segments,
        }
    }

    fn apply_text_replacements(&self, text: &str) -> String {
        if text.is_empty() || self.rules.is_empty() {
            return text.to_string();
        }

        self.rules.iter().fold(text.to_string(), |current, rule| {
            rule.regex
                .replace_all(&current, NoExpand(rule.replacement.as_str()))
                .into_owned()
        })
    }
}

impl Default for TranscriptPostprocessor {
    fn default() -> Self {
        Self::compile(TranscriptPostprocessOptions::default())
            .expect("default transcript postprocess options must compile")
    }
}

#[derive(Clone, Debug)]
struct ActiveTextReplacementRule {
    from: String,
    to: String,
    ignore_case: bool,
}

fn active_text_replacement_rules(
    sets: &[TranscriptTextReplacementRuleSet],
) -> Vec<ActiveTextReplacementRule> {
    let mut rules = sets
        .iter()
        .filter(|set| set.enabled && !set.rules.is_empty())
        .flat_map(|set| {
            set.rules
                .iter()
                .filter(|rule| !rule.from.is_empty())
                .map(|rule| ActiveTextReplacementRule {
                    from: rule.from.clone(),
                    to: rule.to.clone(),
                    ignore_case: set.ignore_case,
                })
        })
        .collect::<Vec<_>>();

    rules.sort_by(|left, right| {
        right
            .from
            .encode_utf16()
            .count()
            .cmp(&left.from.encode_utf16().count())
    });
    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::transcript::{
        TranscriptTiming, TranscriptTimingLevel, TranscriptTimingSource,
    };

    fn sample_segment(text: &str, start: f64, end: f64) -> TranscriptSegment {
        TranscriptSegment {
            id: "segment-1".to_string(),
            text: text.to_string(),
            start,
            end,
            is_final: true,
            timing: Some(TranscriptTiming {
                level: TranscriptTimingLevel::Segment,
                source: TranscriptTimingSource::Derived,
                units: vec![],
            }),
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        }
    }

    #[test]
    fn postprocess_segments_replaces_enabled_rules_like_frontend() {
        let processor = TranscriptPostprocessor::compile(TranscriptPostprocessOptions {
            text_replacement_sets: vec![
                TranscriptTextReplacementRuleSet {
                    enabled: false,
                    ignore_case: false,
                    rules: vec![TranscriptTextReplacementRule {
                        from: "fruit".to_string(),
                        to: "ignored".to_string(),
                    }],
                },
                TranscriptTextReplacementRuleSet {
                    enabled: true,
                    ignore_case: false,
                    rules: vec![
                        TranscriptTextReplacementRule {
                            from: "apples".to_string(),
                            to: "oranges".to_string(),
                        },
                        TranscriptTextReplacementRule {
                            from: "price $5.00?".to_string(),
                            to: "free".to_string(),
                        },
                        TranscriptTextReplacementRule {
                            from: String::new(),
                            to: "empty".to_string(),
                        },
                    ],
                },
                TranscriptTextReplacementRuleSet {
                    enabled: true,
                    ignore_case: true,
                    rules: vec![TranscriptTextReplacementRule {
                        from: "APPLE".to_string(),
                        to: "fruit".to_string(),
                    }],
                },
            ],
            drop_final_dot_segments: false,
        })
        .unwrap();

        let processed = processor.process_segments(vec![sample_segment(
            "APPLE costs price $5.00? and apples",
            0.0,
            1.0,
        )]);

        assert_eq!(processed.len(), 1);
        assert_eq!(processed[0].text, "fruit costs free and oranges");
    }

    #[test]
    fn postprocess_segments_treats_replacement_as_literal_text() {
        let processor = TranscriptPostprocessor::compile(TranscriptPostprocessOptions {
            text_replacement_sets: vec![TranscriptTextReplacementRuleSet {
                enabled: true,
                ignore_case: false,
                rules: vec![TranscriptTextReplacementRule {
                    from: "token".to_string(),
                    to: "$1 literal".to_string(),
                }],
            }],
            drop_final_dot_segments: false,
        })
        .unwrap();

        let processed = processor.process_segments(vec![sample_segment("token", 0.0, 1.0)]);

        assert_eq!(processed[0].text, "$1 literal");
    }

    #[test]
    fn postprocess_segments_drops_only_exact_final_dot_segments() {
        let processor = TranscriptPostprocessor::compile(TranscriptPostprocessOptions {
            text_replacement_sets: Vec::new(),
            drop_final_dot_segments: true,
        })
        .unwrap();
        let mut partial_dot = sample_segment(".", 0.0, 0.5);
        partial_dot.id = "partial-dot".to_string();
        partial_dot.is_final = false;
        let mut final_dot = sample_segment(".", 0.5, 1.0);
        final_dot.id = "final-dot".to_string();
        let mut spaced_dot = sample_segment(" .", 1.0, 1.5);
        spaced_dot.id = "spaced-dot".to_string();

        let processed = processor.process_segments(vec![partial_dot, final_dot, spaced_dot]);

        assert_eq!(
            processed
                .iter()
                .map(|segment| segment.id.as_str())
                .collect::<Vec<_>>(),
            vec!["partial-dot", "spaced-dot"]
        );
    }

    #[test]
    fn postprocess_update_removes_dropped_final_dot_segment_id() {
        let processor = TranscriptPostprocessor::compile(TranscriptPostprocessOptions {
            text_replacement_sets: Vec::new(),
            drop_final_dot_segments: true,
        })
        .unwrap();
        let update = processor.process_update(TranscriptUpdate {
            remove_ids: vec!["existing".to_string()],
            upsert_segments: vec![sample_segment(".", 0.0, 0.5)],
        });

        assert_eq!(
            update.remove_ids,
            vec!["existing".to_string(), "segment-1".to_string()]
        );
        assert!(update.upsert_segments.is_empty());
    }
}
