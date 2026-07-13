use crossterm::{cursor, queue, terminal};
use serde::Serialize;
use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};
use std::io::Write;
use unicode_width::UnicodeWidthStr;

struct SizedWriter<'a, W: Write + ?Sized>(&'a mut W);

impl<W: Write + ?Sized> Write for SizedWriter<'_, W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

#[derive(Debug, Default)]
pub(crate) struct TranscriptAccumulator {
    segments: Vec<TranscriptSegment>,
}

impl TranscriptAccumulator {
    pub(crate) fn apply(&mut self, update: &TranscriptUpdate) {
        self.segments
            .retain(|segment| !update.remove_ids.contains(&segment.id));
        for incoming in &update.upsert_segments {
            if let Some(existing) = self
                .segments
                .iter_mut()
                .find(|segment| segment.id == incoming.id)
            {
                *existing = incoming.clone();
            } else {
                self.segments.push(incoming.clone());
            }
        }
        self.segments.sort_by(|left, right| {
            left.start
                .total_cmp(&right.start)
                .then_with(|| left.end.total_cmp(&right.end))
                .then_with(|| left.id.cmp(&right.id))
        });
    }

    pub(crate) fn segments(&self) -> &[TranscriptSegment] {
        &self.segments
    }

    pub(crate) fn plain_text(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum LiveStopReason {
    CtrlC,
    Eof,
    Duration,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum LiveOutputEvent {
    Started {
        session_id: String,
        source: String,
        device_name: Option<String>,
        sample_rate: u32,
        model_id: String,
    },
    Update {
        session_id: String,
        stage: String,
        remove_ids: Vec<String>,
        upsert_segments: Vec<TranscriptSegment>,
    },
    Stopped {
        session_id: String,
        reason: LiveStopReason,
        segments: Vec<TranscriptSegment>,
    },
    Error {
        session_id: String,
        message: String,
    },
}

pub(crate) fn to_ndjson_line(event: &LiveOutputEvent) -> Result<String, serde_json::Error> {
    serde_json::to_string(event).map(|line| format!("{line}\n"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LiveOutputFormat {
    Text,
    Ndjson,
}

pub(crate) struct LiveOutputRenderer {
    format: LiveOutputFormat,
    terminal: bool,
    session_id: String,
    accumulator: TranscriptAccumulator,
    rendered_lines: usize,
}

impl LiveOutputRenderer {
    pub(crate) fn new(
        format: LiveOutputFormat,
        terminal: bool,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            format,
            terminal,
            session_id: session_id.into(),
            accumulator: TranscriptAccumulator::default(),
            rendered_lines: 0,
        }
    }

    pub(crate) fn write_started<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        source: &str,
        device_name: Option<&str>,
        model_id: &str,
    ) -> Result<(), String> {
        if self.format == LiveOutputFormat::Ndjson {
            self.write_event(
                writer,
                &LiveOutputEvent::Started {
                    session_id: self.session_id.clone(),
                    source: source.to_string(),
                    device_name: device_name.map(str::to_string),
                    sample_rate: TARGET_SAMPLE_RATE,
                    model_id: model_id.to_string(),
                },
            )?;
        }
        Ok(())
    }

    pub(crate) fn write_update<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        stage: &str,
        update: TranscriptUpdate,
    ) -> Result<(), String> {
        self.accumulator.apply(&update);
        match self.format {
            LiveOutputFormat::Ndjson => self.write_event(
                writer,
                &LiveOutputEvent::Update {
                    session_id: self.session_id.clone(),
                    stage: stage.to_string(),
                    remove_ids: update.remove_ids,
                    upsert_segments: update.upsert_segments,
                },
            ),
            LiveOutputFormat::Text if self.terminal => self.refresh_terminal(writer, false),
            LiveOutputFormat::Text => Ok(()),
        }
    }

    pub(crate) fn write_stopped<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        reason: LiveStopReason,
    ) -> Result<(), String> {
        match self.format {
            LiveOutputFormat::Ndjson => self.write_event(
                writer,
                &LiveOutputEvent::Stopped {
                    session_id: self.session_id.clone(),
                    reason,
                    segments: self.accumulator.segments().to_vec(),
                },
            ),
            LiveOutputFormat::Text if self.terminal => self.refresh_terminal(writer, true),
            LiveOutputFormat::Text => {
                let text = self.accumulator.plain_text();
                if !text.is_empty() {
                    writer
                        .write_all(format!("{text}\n").as_bytes())
                        .map_err(|error| format!("Failed to write live transcript: {error}"))?;
                    writer
                        .flush()
                        .map_err(|error| format!("Failed to flush live transcript: {error}"))?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn write_error<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        message: &str,
    ) -> Result<(), String> {
        if self.format == LiveOutputFormat::Ndjson {
            self.write_event(
                writer,
                &LiveOutputEvent::Error {
                    session_id: self.session_id.clone(),
                    message: message.to_string(),
                },
            )?;
        }
        Ok(())
    }

    pub(crate) fn segments(&self) -> &[TranscriptSegment] {
        self.accumulator.segments()
    }

    fn write_event<W: Write + ?Sized>(
        &self,
        writer: &mut W,
        event: &LiveOutputEvent,
    ) -> Result<(), String> {
        let line = to_ndjson_line(event)
            .map_err(|error| format!("Failed to serialize live output: {error}"))?;
        writer
            .write_all(line.as_bytes())
            .map_err(|error| format!("Failed to write live output: {error}"))?;
        writer
            .flush()
            .map_err(|error| format!("Failed to flush live output: {error}"))
    }

    fn refresh_terminal<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        finish: bool,
    ) -> Result<(), String> {
        let mut writer = SizedWriter(writer);
        if self.rendered_lines > 0 {
            if self.rendered_lines > 1 {
                queue!(writer, cursor::MoveUp((self.rendered_lines - 1) as u16))
                    .map_err(|error| format!("Failed to move terminal cursor: {error}"))?;
            }
            queue!(
                writer,
                cursor::MoveToColumn(0),
                terminal::Clear(terminal::ClearType::FromCursorDown)
            )
            .map_err(|error| format!("Failed to clear live transcript: {error}"))?;
        }
        let text = self.accumulator.plain_text();
        writer
            .write_all(text.as_bytes())
            .map_err(|error| format!("Failed to write live transcript: {error}"))?;
        if finish {
            writer
                .write_all(b"\n")
                .map_err(|error| format!("Failed to finish live transcript: {error}"))?;
            self.rendered_lines = 0;
        } else {
            let terminal_columns = terminal::size()
                .map(|(columns, _)| usize::from(columns))
                .unwrap_or(80);
            self.rendered_lines = rendered_terminal_rows(&text, terminal_columns);
        }
        writer
            .flush()
            .map_err(|error| format!("Failed to flush live transcript: {error}"))
    }
}

fn rendered_terminal_rows(text: &str, columns: usize) -> usize {
    let columns = columns.max(1);
    text.split('\n')
        .map(|line| UnicodeWidthStr::width(line).max(1).div_ceil(columns))
        .sum::<usize>()
        .max(1)
}

const TARGET_SAMPLE_RATE: u32 = 16_000;

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};

    fn segment(id: &str, text: &str, start: f64, end: f64, is_final: bool) -> TranscriptSegment {
        TranscriptSegment {
            id: id.to_string(),
            text: text.to_string(),
            start,
            end,
            is_final,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        }
    }

    #[test]
    fn accumulator_applies_remove_upsert_and_stable_timing_order() {
        let mut accumulator = TranscriptAccumulator::default();
        accumulator.apply(&TranscriptUpdate {
            remove_ids: Vec::new(),
            upsert_segments: vec![
                segment("later", "world", 2.0, 3.0, false),
                segment("first", "hello", 0.0, 1.0, true),
            ],
        });

        assert_eq!(
            accumulator
                .segments()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "later"]
        );

        accumulator.apply(&TranscriptUpdate {
            remove_ids: vec!["first".to_string()],
            upsert_segments: vec![segment("later", "world!", 2.0, 3.2, true)],
        });

        assert_eq!(accumulator.segments().len(), 1);
        assert_eq!(accumulator.segments()[0].text, "world!");
        assert_eq!(accumulator.plain_text(), "world!");
    }

    #[test]
    fn ndjson_events_use_the_stable_public_contract() {
        let started = LiveOutputEvent::Started {
            session_id: "session-1".to_string(),
            source: "microphone".to_string(),
            device_name: Some("Studio Mic".to_string()),
            sample_rate: 16_000,
            model_id: "streaming-model".to_string(),
        };
        let update = LiveOutputEvent::Update {
            session_id: "session-1".to_string(),
            stage: "partial".to_string(),
            remove_ids: vec!["old".to_string()],
            upsert_segments: vec![segment("new", "hello", 0.0, 1.0, false)],
        };
        let stopped = LiveOutputEvent::Stopped {
            session_id: "session-1".to_string(),
            reason: LiveStopReason::CtrlC,
            segments: vec![segment("new", "hello", 0.0, 1.0, true)],
        };
        let error = LiveOutputEvent::Error {
            session_id: "session-1".to_string(),
            message: "device lost".to_string(),
        };

        assert_eq!(
            serde_json::to_value(started).unwrap(),
            serde_json::json!({
                "type": "started",
                "sessionId": "session-1",
                "source": "microphone",
                "deviceName": "Studio Mic",
                "sampleRate": 16000,
                "modelId": "streaming-model"
            })
        );
        let update_value = serde_json::to_value(update).unwrap();
        assert_eq!(update_value["type"], "update");
        assert_eq!(update_value["sessionId"], "session-1");
        assert_eq!(update_value["stage"], "partial");
        assert_eq!(update_value["removeIds"], serde_json::json!(["old"]));
        assert_eq!(update_value["upsertSegments"][0]["id"], "new");
        assert_eq!(serde_json::to_value(stopped).unwrap()["reason"], "ctrl-c");
        assert_eq!(serde_json::to_value(error).unwrap()["type"], "error");
        assert!(
            to_ndjson_line(&LiveOutputEvent::Error {
                session_id: "session-1".to_string(),
                message: "device lost".to_string(),
            })
            .unwrap()
            .ends_with('\n')
        );
    }

    #[test]
    fn non_terminal_text_buffers_updates_until_stopped() {
        let mut renderer = LiveOutputRenderer::new(LiveOutputFormat::Text, false, "session-1");
        let mut output = Vec::new();

        renderer
            .write_update(
                &mut output,
                "partial",
                TranscriptUpdate {
                    remove_ids: Vec::new(),
                    upsert_segments: vec![segment("one", "hello", 0.0, 1.0, false)],
                },
            )
            .unwrap();
        assert!(output.is_empty());

        renderer
            .write_stopped(&mut output, LiveStopReason::Eof)
            .unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "hello\n");
    }

    #[test]
    fn ndjson_renderer_flushes_each_lifecycle_event_as_one_line() {
        let mut renderer = LiveOutputRenderer::new(LiveOutputFormat::Ndjson, false, "session-1");
        let mut output = Vec::new();

        renderer
            .write_started(&mut output, "stdin", None, "streaming-model")
            .unwrap();
        renderer
            .write_update(
                &mut output,
                "final",
                TranscriptUpdate {
                    remove_ids: Vec::new(),
                    upsert_segments: vec![segment("one", "hello", 0.0, 1.0, true)],
                },
            )
            .unwrap();
        renderer
            .write_stopped(&mut output, LiveStopReason::Duration)
            .unwrap();

        let lines = String::from_utf8(output).unwrap();
        let values = lines
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 3);
        assert_eq!(
            values[0],
            serde_json::json!({
                "type": "started",
                "sessionId": "session-1",
                "source": "stdin",
                "deviceName": null,
                "sampleRate": 16000,
                "modelId": "streaming-model"
            })
        );
        assert_eq!(values[1]["type"], "update");
        assert_eq!(values[2]["type"], "stopped");
        assert_eq!(values[2]["reason"], "duration");
        assert_eq!(values[2]["segments"][0]["text"], "hello");
    }

    #[test]
    fn terminal_text_refreshes_and_finishes_with_a_newline() {
        let mut renderer = LiveOutputRenderer::new(LiveOutputFormat::Text, true, "session-1");
        let mut output = Vec::new();
        renderer
            .write_update(
                &mut output,
                "partial",
                TranscriptUpdate {
                    remove_ids: Vec::new(),
                    upsert_segments: vec![segment("one", "hello", 0.0, 1.0, false)],
                },
            )
            .unwrap();
        renderer
            .write_update(
                &mut output,
                "final",
                TranscriptUpdate {
                    remove_ids: Vec::new(),
                    upsert_segments: vec![segment("one", "hello world", 0.0, 1.0, true)],
                },
            )
            .unwrap();
        renderer
            .write_stopped(&mut output, LiveStopReason::CtrlC)
            .unwrap();

        let rendered = String::from_utf8(output).unwrap();
        assert!(rendered.contains("hello"));
        assert!(rendered.contains("hello world"));
        assert!(rendered.ends_with('\n'));
    }

    #[test]
    fn terminal_row_count_includes_wrapped_display_width() {
        assert_eq!(rendered_terminal_rows("abcdef", 4), 2);
        assert_eq!(rendered_terminal_rows("ab\n12345", 4), 3);
        assert_eq!(rendered_terminal_rows("你好", 3), 2);
        assert_eq!(rendered_terminal_rows("", 4), 1);
    }
}
