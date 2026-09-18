import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { speakerCorrectionService } from '../../../services/speakerCorrectionService';
import { speakerService } from '../../../services/speakerService';
import { useConfigStore } from '../../../stores/configStore';
import { useDialogStore } from '../../../stores/dialogStore';
import type { TranscriptSegment } from '../../../types/transcript';
import { EnrollSpeakerSampleModal } from '../EnrollSpeakerSampleModal';

vi.mock('../../../services/speakerCorrectionService', () => ({
  speakerCorrectionService: {
    assignProfileToSpeakerGroup: vi.fn(),
  },
}));

vi.mock('../../../services/speakerService', () => ({
  speakerService: {
    enrollProfileSampleFromAudio: vi.fn(),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, params?: { defaultValue?: string; name?: string; count?: number }) => {
      if (params?.name) return `Enrolled in ${params.name}`;
      return params?.defaultValue ?? key;
    },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

const mockSegment: TranscriptSegment = {
  id: 'seg-1',
  text: 'Hello, this is a clear reference sentence.',
  start: 10.5,
  end: 15.2,
  isFinal: true,
  speaker: {
    id: 'spk-alice',
    label: 'Alice',
    kind: 'identified',
  },
};

describe('EnrollSpeakerSampleModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        speakerProfiles: [
          {
            id: 'spk-alice',
            name: 'Alice',
            enabled: true,
            samples: [],
          },
          {
            id: 'spk-bob',
            name: 'Bob',
            enabled: true,
            samples: [],
          },
        ],
      },
    });
  });

  it('renders segment text and duration when open', () => {
    render(
      <EnrollSpeakerSampleModal
        isOpen
        onClose={vi.fn()}
        segment={mockSegment}
        audioPath="/path/to/meeting.wav"
      />
    );

    expect(screen.getByText(/Hello, this is a clear reference sentence/)).toBeDefined();
    expect(screen.getByText('4.7s')).toBeDefined();
    expect(screen.getByText('已有说话人')).toBeDefined();
    expect(screen.getByText('新建说话人')).toBeDefined();
  });

  it('submits enrollment for an existing speaker profile', async () => {
    const onClose = vi.fn();
    const alert = vi.fn().mockResolvedValue(undefined);
    useDialogStore.setState({ alert: alert as any });

    vi.mocked(speakerService.enrollProfileSampleFromAudio).mockResolvedValue({
      id: 'sample-1',
      sourceName: 'Sample',
      filePath: '/samples/sample-1.wav',
      durationSeconds: 4.7,
    });

    render(
      <EnrollSpeakerSampleModal
        isOpen
        onClose={onClose}
        segment={mockSegment}
        audioPath="/path/to/meeting.wav"
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '确定录入' }));
    await waitFor(() => {
      expect(speakerService.enrollProfileSampleFromAudio).toHaveBeenCalledWith(
        'spk-alice',
        '/path/to/meeting.wav',
        10.5,
        15.2,
        expect.any(String)
      );
      expect(onClose).toHaveBeenCalled();
      expect(alert).toHaveBeenCalledWith(
        expect.stringContaining('Alice'),
        expect.objectContaining({ variant: 'success' })
      );
    });

    // Check store was updated
    const aliceProfile = useConfigStore
      .getState()
      .config.speakerProfiles?.find((p) => p.id === 'spk-alice');
    expect(aliceProfile?.samples).toHaveLength(1);
  });

  it('creates and enrolls into a new profile', async () => {
    const onClose = vi.fn();
    const alert = vi.fn().mockResolvedValue(undefined);
    useDialogStore.setState({ alert: alert as any });
    vi.mocked(speakerService.enrollProfileSampleFromAudio).mockResolvedValue({
      id: 'sample-2',
      sourceName: 'Charlie Sample',
      filePath: '/samples/sample-2.wav',
      durationSeconds: 4.7,
    });

    render(
      <EnrollSpeakerSampleModal
        isOpen
        onClose={onClose}
        segment={mockSegment}
        audioPath="/path/to/meeting.wav"
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '新建说话人' }));

    const input = screen.getByPlaceholderText('新说话人姓名');
    fireEvent.change(input, { target: { value: 'Charlie' } });

    fireEvent.click(screen.getByRole('button', { name: '确定录入' }));
    await waitFor(() => {
      expect(speakerService.enrollProfileSampleFromAudio).toHaveBeenCalledWith(
        expect.any(String),
        '/path/to/meeting.wav',
        10.5,
        15.2,
        expect.any(String)
      );
      expect(onClose).toHaveBeenCalled();
    });

    const charlie = useConfigStore
      .getState()
      .config.speakerProfiles?.find((p) => p.name === 'Charlie');
    expect(charlie?.samples).toHaveLength(1);
  });
  it('shows error banner and disables submit when audio path is missing', () => {
    render(
      <EnrollSpeakerSampleModal isOpen onClose={vi.fn()} segment={mockSegment} audioPath={null} />
    );

    expect(screen.getByText('当前会话未找到音频文件，无法提取声纹')).toBeDefined();
    expect((screen.getByRole('button', { name: '确定录入' }) as HTMLButtonElement).disabled).toBe(
      true
    );
    expect(speakerService.enrollProfileSampleFromAudio).not.toHaveBeenCalled();
  });

  it('auto-assigns profile to speaker group when segment has speakerAttribution', async () => {
    const onClose = vi.fn();
    const alert = vi.fn().mockResolvedValue(undefined);
    useDialogStore.setState({ alert: alert as any });

    vi.mocked(speakerService.enrollProfileSampleFromAudio).mockResolvedValue({
      id: 'sample-99',
      sourceName: 'Group Sample',
      filePath: '/samples/sample-99.wav',
      durationSeconds: 4.7,
    });

    const segmentWithGroup: TranscriptSegment = {
      ...mockSegment,
      speakerAttribution: {
        groupId: 'anonymous-group-1',
        anonymousLabel: 'Speaker 1',
        state: 'anonymous',
        source: 'auto',
        confidence: 'low',
        candidates: [],
      },
    };

    render(
      <EnrollSpeakerSampleModal
        isOpen
        onClose={onClose}
        segment={segmentWithGroup}
        audioPath="/path/to/meeting.wav"
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '确定录入' }));
    await waitFor(() => {
      expect(speakerService.enrollProfileSampleFromAudio).toHaveBeenCalled();
      expect(speakerCorrectionService.assignProfileToSpeakerGroup).toHaveBeenCalledWith(
        'anonymous-group-1',
        'spk-alice'
      );
      expect(onClose).toHaveBeenCalled();
    });
  });

  it('shows warning banner and warning tag when segment duration is under 4 seconds', () => {
    const shortSegment: TranscriptSegment = {
      ...mockSegment,
      start: 10.0,
      end: 12.5,
    };

    render(
      <EnrollSpeakerSampleModal
        isOpen
        onClose={vi.fn()}
        segment={shortSegment}
        audioPath="/path/to/meeting.wav"
      />
    );

    expect(screen.getByText('2.5s')).toBeDefined();
    expect(screen.getByRole('status')).toBeDefined();
    expect(
      screen.getByText(
        '当前片段时长不足 4 秒。声纹引擎将忽略 4 秒以下的样本，建议选择更长的语音片段进行录入。'
      )
    ).toBeDefined();
  });
});
