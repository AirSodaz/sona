#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollDetachMode {
    Streaming,
    Playback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollTarget {
    Bottom,
    Segment(usize),
}

pub fn is_near_scroll_bottom(offset_y: f32, max_offset_y: f32) -> bool {
    -offset_y >= max_offset_y - 60.0
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScrollFollowState {
    streaming_active: bool,
    streaming_follow: bool,
    playback_active: bool,
    playback_follow: bool,
    editing: bool,
    active_segment: Option<usize>,
}

impl ScrollFollowState {
    pub fn reset_session(&mut self) {
        *self = Self::default();
    }

    pub fn set_streaming_active(&mut self, active: bool) -> bool {
        let started = active && !self.streaming_active;
        self.streaming_active = active;
        if started {
            self.streaming_follow = true;
        }
        started
    }

    pub fn update_playback(
        &mut self,
        playing: bool,
        seeked: bool,
        editing: bool,
        active_segment: Option<usize>,
    ) -> Option<ScrollTarget> {
        let started = playing && !self.playback_active;
        self.playback_active = playing;
        self.editing = editing;

        if editing && playing {
            self.playback_follow = false;
            self.active_segment = active_segment;
            return None;
        }

        if started || (seeked && !editing) {
            self.playback_follow = true;
            self.active_segment = active_segment;
            return active_segment.map(ScrollTarget::Segment);
        }

        let changed = self.active_segment != active_segment;
        self.active_segment = active_segment;
        if changed && playing && self.playback_follow && !editing {
            return active_segment.map(ScrollTarget::Segment);
        }
        None
    }

    pub fn on_user_scroll(&mut self, at_bottom: bool) {
        if self.streaming_active {
            self.streaming_follow = at_bottom;
        }
        if self.playback_active {
            self.playback_follow = false;
        }
    }

    pub fn resume(&mut self) -> Option<ScrollTarget> {
        if self.streaming_active {
            self.streaming_follow = true;
            Some(ScrollTarget::Bottom)
        } else if self.playback_active {
            self.playback_follow = true;
            self.active_segment.map(ScrollTarget::Segment)
        } else {
            None
        }
    }

    pub fn is_following(&self) -> bool {
        if self.streaming_active {
            self.streaming_follow
        } else if self.playback_active {
            self.playback_follow
        } else {
            true
        }
    }

    pub fn is_streaming_following(&self) -> bool {
        self.streaming_active && self.streaming_follow
    }

    pub fn detach_mode(&self) -> Option<ScrollDetachMode> {
        if self.streaming_active && !self.streaming_follow {
            Some(ScrollDetachMode::Streaming)
        } else if self.playback_active && !self.playback_follow {
            Some(ScrollDetachMode::Playback)
        } else {
            None
        }
    }
}

pub fn transcript_word_count<'a>(segments: impl IntoIterator<Item = &'a str>) -> usize {
    let mut count = 0;
    let mut in_word = false;
    for character in segments.into_iter().flat_map(str::chars) {
        if is_cjk(character) {
            count += 1;
            in_word = false;
        } else if character.is_alphanumeric() {
            if !in_word {
                count += 1;
            }
            in_word = true;
        } else {
            in_word = false;
        }
    }
    count
}

fn is_cjk(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x3040..=0x309F
            | 0x30A0..=0x30FF
            | 0x31F0..=0x31FF
            | 0xAC00..=0xD7AF
            | 0x1100..=0x11FF
            | 0x3130..=0x318F
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ScrollDetachMode, ScrollFollowState, ScrollTarget, is_near_scroll_bottom,
        transcript_word_count,
    };

    #[test]
    fn scroll_bottom_detection_uses_actual_offset_and_react_threshold() {
        assert!(!is_near_scroll_bottom(0.0, 500.0));
        assert!(!is_near_scroll_bottom(-439.0, 500.0));
        assert!(is_near_scroll_bottom(-440.0, 500.0));
        assert!(is_near_scroll_bottom(0.0, 0.0));
    }

    #[test]
    fn streaming_detaches_and_reattaches_at_bottom() {
        let mut state = ScrollFollowState::default();
        assert!(state.set_streaming_active(true));
        state.on_user_scroll(false);
        assert_eq!(state.detach_mode(), Some(ScrollDetachMode::Streaming));
        state.on_user_scroll(true);
        assert_eq!(state.detach_mode(), None);
    }

    #[test]
    fn playback_detaches_on_manual_scroll_and_resumes_at_active_segment() {
        let mut state = ScrollFollowState::default();
        assert_eq!(
            state.update_playback(true, false, false, Some(4)),
            Some(ScrollTarget::Segment(4))
        );
        state.on_user_scroll(false);
        assert_eq!(state.detach_mode(), Some(ScrollDetachMode::Playback));
        assert_eq!(state.resume(), Some(ScrollTarget::Segment(4)));
        assert_eq!(state.detach_mode(), None);
    }

    #[test]
    fn playback_seek_resumes_follow_and_editing_detaches() {
        let mut state = ScrollFollowState::default();
        state.update_playback(true, false, false, Some(2));
        state.on_user_scroll(false);
        assert_eq!(
            state.update_playback(true, true, false, Some(7)),
            Some(ScrollTarget::Segment(7))
        );
        assert_eq!(state.update_playback(true, false, true, Some(7)), None);
        assert_eq!(state.detach_mode(), Some(ScrollDetachMode::Playback));
    }

    #[test]
    fn session_and_stream_restart_restore_tail_follow() {
        let mut state = ScrollFollowState::default();
        state.set_streaming_active(true);
        state.on_user_scroll(false);
        state.set_streaming_active(false);
        state.reset_session();
        assert!(state.set_streaming_active(true));
        assert!(state.is_following());
    }

    #[test]
    fn word_count_matches_cjk_characters_and_latin_number_runs() {
        assert_eq!(transcript_word_count(["Hello, Sona 42 你好かな한글!"]), 9);
    }
}
