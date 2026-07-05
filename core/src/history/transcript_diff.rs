use std::collections::{HashMap, HashSet};

use crate::transcript::{TranscriptSegment, ensure_transcript_segment_timing};

use super::{TranscriptDiffResult, TranscriptDiffRow, TranscriptDiffStatus};

const TIME_MATCH_TOLERANCE_SECONDS: f64 = 1.5;

fn normalize_segments(mut segments: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    for segment in &mut segments {
        ensure_transcript_segment_timing(segment);
    }
    segments
}

fn is_same_segment_content(
    snapshot_segment: Option<&TranscriptSegment>,
    current_segment: Option<&TranscriptSegment>,
) -> bool {
    let Some(snapshot_segment) = snapshot_segment else {
        return current_segment.is_none();
    };
    let Some(current_segment) = current_segment else {
        return false;
    };

    snapshot_segment.end == current_segment.end
        && snapshot_segment.is_final == current_segment.is_final
        && snapshot_segment.speaker == current_segment.speaker
        && snapshot_segment.speaker_attribution == current_segment.speaker_attribution
        && snapshot_segment.start == current_segment.start
        && snapshot_segment.text == current_segment.text
        && snapshot_segment.timing == current_segment.timing
        && snapshot_segment.translation.as_deref().unwrap_or_default()
            == current_segment.translation.as_deref().unwrap_or_default()
}

fn segment_time_distance(
    snapshot_segment: &TranscriptSegment,
    current_segment: &TranscriptSegment,
) -> f64 {
    (snapshot_segment.start - current_segment.start).abs()
        + (snapshot_segment.end - current_segment.end).abs()
}

fn find_best_time_match(
    snapshot_segment: &TranscriptSegment,
    current_segments: &[TranscriptSegment],
    used_current_indexes: &HashSet<usize>,
) -> Option<usize> {
    let mut best_index = None;
    let mut best_distance = f64::INFINITY;

    for (index, current_segment) in current_segments.iter().enumerate() {
        if used_current_indexes.contains(&index) {
            continue;
        }

        let distance = segment_time_distance(snapshot_segment, current_segment);
        if distance <= TIME_MATCH_TOLERANCE_SECONDS && distance < best_distance {
            best_distance = distance;
            best_index = Some(index);
        }
    }

    best_index
}

fn build_row_id(snapshot_index: Option<usize>, current_index: Option<usize>) -> String {
    format!(
        "diff-{}-{}",
        snapshot_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "x".to_string()),
        current_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "x".to_string())
    )
}

fn match_segments(
    snapshot_segments: &[TranscriptSegment],
    current_segments: &[TranscriptSegment],
) -> HashMap<usize, usize> {
    let mut matches = HashMap::<usize, usize>::new();
    let mut used_current_indexes = HashSet::<usize>::new();

    match_segments_by_id(
        snapshot_segments,
        current_segments,
        &mut matches,
        &mut used_current_indexes,
    );
    match_segments_by_time(
        snapshot_segments,
        current_segments,
        &mut matches,
        &mut used_current_indexes,
    );
    match_remaining_segments_by_order(
        snapshot_segments,
        current_segments,
        &mut matches,
        &mut used_current_indexes,
    );

    matches
}

fn match_segments_by_id(
    snapshot_segments: &[TranscriptSegment],
    current_segments: &[TranscriptSegment],
    matches: &mut HashMap<usize, usize>,
    used_current_indexes: &mut HashSet<usize>,
) {
    let current_index_by_id = current_segments
        .iter()
        .enumerate()
        .map(|(index, segment)| (segment.id.clone(), index))
        .collect::<HashMap<_, _>>();

    for (snapshot_index, snapshot_segment) in snapshot_segments.iter().enumerate() {
        let Some(current_index) = current_index_by_id.get(&snapshot_segment.id).copied() else {
            continue;
        };
        if used_current_indexes.contains(&current_index) {
            continue;
        }

        matches.insert(snapshot_index, current_index);
        used_current_indexes.insert(current_index);
    }
}

fn match_segments_by_time(
    snapshot_segments: &[TranscriptSegment],
    current_segments: &[TranscriptSegment],
    matches: &mut HashMap<usize, usize>,
    used_current_indexes: &mut HashSet<usize>,
) {
    for (snapshot_index, snapshot_segment) in snapshot_segments.iter().enumerate() {
        if matches.contains_key(&snapshot_index) {
            continue;
        }

        let Some(current_index) =
            find_best_time_match(snapshot_segment, current_segments, used_current_indexes)
        else {
            continue;
        };

        matches.insert(snapshot_index, current_index);
        used_current_indexes.insert(current_index);
    }
}

fn match_remaining_segments_by_order(
    snapshot_segments: &[TranscriptSegment],
    current_segments: &[TranscriptSegment],
    matches: &mut HashMap<usize, usize>,
    used_current_indexes: &mut HashSet<usize>,
) {
    let mut next_unmatched_current_index = 0;

    for snapshot_index in 0..snapshot_segments.len() {
        if matches.contains_key(&snapshot_index) {
            continue;
        }

        while next_unmatched_current_index < current_segments.len()
            && used_current_indexes.contains(&next_unmatched_current_index)
        {
            next_unmatched_current_index += 1;
        }

        if next_unmatched_current_index >= current_segments.len() {
            break;
        }

        matches.insert(snapshot_index, next_unmatched_current_index);
        used_current_indexes.insert(next_unmatched_current_index);
        next_unmatched_current_index += 1;
    }
}

fn build_diff_rows(
    snapshot_segments: &[TranscriptSegment],
    current_segments: &[TranscriptSegment],
    matches: &HashMap<usize, usize>,
) -> Vec<TranscriptDiffRow> {
    let mut rows = Vec::new();
    let mut final_matched_current_indexes = HashSet::new();

    for (snapshot_index, snapshot_segment) in snapshot_segments.iter().enumerate() {
        let current_index = matches.get(&snapshot_index);

        let Some(&current_index) = current_index else {
            rows.push(removed_row(snapshot_index, snapshot_segment));
            continue;
        };

        final_matched_current_indexes.insert(current_index);

        rows.push(matched_row(
            snapshot_index,
            snapshot_segment,
            current_index,
            &current_segments[current_index],
        ));
    }

    for (current_index, current_segment) in current_segments.iter().enumerate() {
        if final_matched_current_indexes.contains(&current_index) {
            continue;
        }

        rows.push(added_row(current_index, current_segment));
    }

    rows.sort_by(|left, right| {
        let left_time = left
            .current_segment
            .as_ref()
            .or(left.snapshot_segment.as_ref())
            .map(|s| s.start)
            .unwrap_or(0.0);
        let right_time = right
            .current_segment
            .as_ref()
            .or(right.snapshot_segment.as_ref())
            .map(|s| s.start)
            .unwrap_or(0.0);
        left_time
            .partial_cmp(&right_time)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.id.cmp(&right.id))
    });
    rows
}

fn matched_row(
    snapshot_index: usize,
    snapshot_segment: &TranscriptSegment,
    current_index: usize,
    current_segment: &TranscriptSegment,
) -> TranscriptDiffRow {
    let status = if is_same_segment_content(Some(snapshot_segment), Some(current_segment)) {
        TranscriptDiffStatus::Unchanged
    } else {
        TranscriptDiffStatus::Modified
    };

    TranscriptDiffRow {
        id: build_row_id(Some(snapshot_index), Some(current_index)),
        status,
        snapshot_segment: Some(snapshot_segment.clone()),
        current_segment: Some(current_segment.clone()),
        snapshot_index: Some(snapshot_index),
        current_index: Some(current_index),
    }
}

fn removed_row(snapshot_index: usize, snapshot_segment: &TranscriptSegment) -> TranscriptDiffRow {
    TranscriptDiffRow {
        id: build_row_id(Some(snapshot_index), None),
        status: TranscriptDiffStatus::Removed,
        snapshot_segment: Some(snapshot_segment.clone()),
        current_segment: None,
        snapshot_index: Some(snapshot_index),
        current_index: None,
    }
}

fn added_row(current_index: usize, current_segment: &TranscriptSegment) -> TranscriptDiffRow {
    TranscriptDiffRow {
        id: build_row_id(None, Some(current_index)),
        status: TranscriptDiffStatus::Added,
        snapshot_segment: None,
        current_segment: Some(current_segment.clone()),
        snapshot_index: None,
        current_index: Some(current_index),
    }
}

fn changed_row_count(rows: &[TranscriptDiffRow]) -> usize {
    rows.iter()
        .filter(|row| row.status != TranscriptDiffStatus::Unchanged)
        .count()
}

pub fn build_transcript_diff(
    snapshot_segments: Vec<TranscriptSegment>,
    current_segments: Vec<TranscriptSegment>,
) -> TranscriptDiffResult {
    let snapshot_segments = normalize_segments(snapshot_segments);
    let current_segments = normalize_segments(current_segments);
    let matches = match_segments(&snapshot_segments, &current_segments);
    let rows = build_diff_rows(&snapshot_segments, &current_segments, &matches);
    let changed_count = changed_row_count(&rows);

    TranscriptDiffResult {
        rows,
        changed_count,
    }
}

pub fn restore_transcript_diff_rows(
    rows: Vec<TranscriptDiffRow>,
    selected_row_ids: Vec<String>,
) -> Vec<TranscriptSegment> {
    let selected_row_ids = selected_row_ids.into_iter().collect::<HashSet<_>>();
    let mut next_segments = Vec::new();

    for row in rows {
        let selected = selected_row_ids.contains(&row.id);
        if selected {
            if row.status == TranscriptDiffStatus::Added {
                continue;
            }

            if let Some(snapshot_segment) = row.snapshot_segment {
                next_segments.push(snapshot_segment);
            }
            continue;
        }

        if let Some(current_segment) = row.current_segment {
            next_segments.push(current_segment);
        }
    }

    normalize_segments(next_segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(id: &str, text: &str, start: f64, end: f64) -> TranscriptSegment {
        TranscriptSegment {
            id: id.to_string(),
            text: text.to_string(),
            start,
            end,
            is_final: true,
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
    fn transcript_diff_matches_segments_by_id_first() {
        let result = build_transcript_diff(
            vec![segment("same", "before", 0.0, 1.0)],
            vec![segment("same", "after", 10.0, 11.0)],
        );

        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].id, "diff-0-0");
        assert_eq!(result.rows[0].status, TranscriptDiffStatus::Modified);
        assert_eq!(result.changed_count, 1);
    }

    #[test]
    fn transcript_diff_uses_time_match_within_tolerance() {
        let result = build_transcript_diff(
            vec![segment("snapshot", "hello", 1.0, 2.0)],
            vec![segment("current", "hello", 1.3, 2.2)],
        );

        assert_eq!(result.rows[0].id, "diff-0-0");
        assert_eq!(result.rows[0].status, TranscriptDiffStatus::Modified);
    }

    #[test]
    fn transcript_diff_treats_missing_and_empty_translation_as_same_content() {
        let mut current = segment("same", "hello", 0.0, 1.0);
        current.translation = Some(String::new());

        let result = build_transcript_diff(vec![segment("same", "hello", 0.0, 1.0)], vec![current]);

        assert_eq!(result.rows[0].status, TranscriptDiffStatus::Unchanged);
        assert_eq!(result.changed_count, 0);
    }

    #[test]
    fn transcript_diff_pairs_remaining_segments_by_order() {
        let result = build_transcript_diff(
            vec![segment("snapshot-a", "a", 0.0, 1.0)],
            vec![segment("current-a", "a", 10.0, 11.0)],
        );

        assert_eq!(result.rows[0].id, "diff-0-0");
        assert_eq!(result.rows[0].status, TranscriptDiffStatus::Modified);
    }

    #[test]
    fn transcript_diff_reports_added_and_removed_rows() {
        let added = build_transcript_diff(
            vec![segment("same", "same", 0.0, 1.0)],
            vec![
                segment("same", "same", 0.0, 1.0),
                segment("added", "added", 2.0, 3.0),
            ],
        );
        assert_eq!(added.rows[0].status, TranscriptDiffStatus::Unchanged);
        assert_eq!(added.rows[1].status, TranscriptDiffStatus::Added);
        assert_eq!(added.changed_count, 1);

        let removed = build_transcript_diff(
            vec![
                segment("same", "same", 0.0, 1.0),
                segment("removed", "removed", 2.0, 3.0),
            ],
            vec![segment("same", "same", 0.0, 1.0)],
        );
        assert_eq!(removed.rows[0].status, TranscriptDiffStatus::Unchanged);
        assert_eq!(removed.rows[1].status, TranscriptDiffStatus::Removed);
        assert_eq!(removed.changed_count, 1);
    }

    #[test]
    fn transcript_diff_changed_count_excludes_unchanged_rows() {
        let result = build_transcript_diff(
            vec![
                segment("same", "same", 0.0, 1.0),
                segment("changed", "before", 2.0, 3.0),
            ],
            vec![
                segment("same", "same", 0.0, 1.0),
                segment("changed", "after", 2.0, 3.0),
                segment("added", "added", 4.0, 5.0),
            ],
        );

        assert_eq!(result.changed_count, 2);
    }

    #[test]
    fn restore_transcript_diff_rows_replaces_selected_rows_and_drops_selected_added_rows() {
        let result = build_transcript_diff(
            vec![segment("same", "snapshot", 0.0, 1.0)],
            vec![
                segment("same", "current", 0.0, 1.0),
                segment("added", "added", 2.0, 3.0),
            ],
        );
        let selected = result
            .rows
            .iter()
            .map(|row| row.id.clone())
            .collect::<Vec<_>>();

        let restored = restore_transcript_diff_rows(result.rows, selected);

        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].text, "snapshot");
    }

    #[test]
    fn transcript_diff_out_of_order_matching_sorts_correctly() {
        let result = build_transcript_diff(
            vec![
                segment("A", "A_snap", 0.0, 1.0),
                segment("B", "B_snap", 2.0, 3.0),
            ],
            vec![
                segment("B", "B_curr", 2.0, 3.0),
                segment("A", "A_curr", 0.0, 1.0),
            ],
        );

        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].id, "diff-0-1"); // A's row: snapshot_index 0, current_index 1 (starts at 0.0)
        assert_eq!(result.rows[1].id, "diff-1-0"); // B's row: snapshot_index 1, current_index 0 (starts at 2.0)
    }
}
