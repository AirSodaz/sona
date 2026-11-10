import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import { useTranscriptStore } from '../../../test-utils/transcriptStoreTestUtils';
import type { TranscriptSegment } from '../../../types/transcript';
import { TranscriptAnticipationIndicator } from '../TranscriptAnticipationIndicator';

describe('TranscriptAnticipationIndicator', () => {
  beforeEach(() => {
    useTranscriptStore.setState({
      isRecording: false,
      isPaused: false,
      segments: [],
    });
  });

  it('renders nothing when not recording', () => {
    const { container } = render(<TranscriptAnticipationIndicator />);
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing when recording is paused', () => {
    useTranscriptStore.setState({
      isRecording: true,
      isPaused: true,
      segments: [],
    });

    const { container } = render(<TranscriptAnticipationIndicator />);
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing when there are no segments', () => {
    useTranscriptStore.setState({
      isRecording: true,
      isPaused: false,
      segments: [],
    });

    const { container } = render(<TranscriptAnticipationIndicator />);
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing when the last segment is still streaming partial tokens', () => {
    const partialSegment: TranscriptSegment = {
      id: 'seg-1',
      start: 0,
      end: 2.5,
      text: '实时输入中',
      isFinal: false,
    };

    useTranscriptStore.setState({
      isRecording: true,
      isPaused: false,
      segments: [partialSegment],
    });

    const { container } = render(<TranscriptAnticipationIndicator />);
    expect(container.firstChild).toBeNull();
  });

  it('renders the subtle next-segment row with breathing caret when the last segment is finalized', () => {
    const finalizedSegment: TranscriptSegment = {
      id: 'seg-1',
      start: 0,
      end: 4.2,
      text: '第一句已结束。',
      isFinal: true,
    };

    useTranscriptStore.setState({
      isRecording: true,
      isPaused: false,
      segments: [finalizedSegment],
    });

    const { container } = render(<TranscriptAnticipationIndicator />);
    expect(screen.getByText('00:04.2')).toBeTruthy();
    expect(container.querySelector('.transcript-caret-pulse')).toBeTruthy();
  });

  it('renders nothing when active session is not the recording session during background recording', () => {
    const finalizedSegment: TranscriptSegment = {
      id: 'seg-1',
      start: 0,
      end: 4.2,
      text: '已保存记录',
      isFinal: true,
    };

    useTranscriptStore.setState({
      isRecording: true,
      recordingSessionId: 'active-rec-session',
      activeSessionId: 'different-viewed-session',
      isPaused: false,
      segments: [finalizedSegment],
    });

    const { container } = render(<TranscriptAnticipationIndicator />);
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing on completed historical record even if recordingSessionId is unset during recording', () => {
    const finalizedSegment: TranscriptSegment = {
      id: 'seg-1',
      start: 0,
      end: 4.2,
      text: '已完成的历史录音',
      isFinal: true,
    };

    useTranscriptStore.setState({
      isRecording: true,
      recordingSessionId: null,
      sourceHistoryId: 'historical-item-456',
      isPaused: false,
      segments: [finalizedSegment],
    });

    const { container } = render(<TranscriptAnticipationIndicator />);
    expect(container.firstChild).toBeNull();
  });
});
