import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { FirstRunGuide } from '../FirstRunGuide';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { useOnboardingStore } from '../../stores/onboardingStore';
import {
  downloadRecommendedOnboardingModels,
  getRecommendedOnboardingConfig,
  getRecommendedOnboardingModels,
} from '../../services/onboardingService';
import {
  listMicrophoneDeviceOptions,
  requestMicrophonePermission,
} from '../../services/audioDeviceService';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('../Dropdown', () => ({
  Dropdown: ({
    id,
    value,
    onChange,
    options,
  }: {
    id: string;
    value: string;
    onChange: (value: string) => void;
    options: Array<{ label: string; value: string }>;
  }) => (
    <select
      aria-label={id}
      value={value}
      onChange={(event) => onChange(event.target.value)}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

vi.mock('../../hooks/useFocusTrap', () => ({
  useFocusTrap: vi.fn(),
}));

vi.mock('../../services/onboardingService', () => ({
  getRecommendedOnboardingModels: vi.fn(),
  downloadRecommendedOnboardingModels: vi.fn(),
  getRecommendedOnboardingConfig: vi.fn(),
}));

vi.mock('../../services/audioDeviceService', () => ({
  requestMicrophonePermission: vi.fn(),
  listMicrophoneDeviceOptions: vi.fn(),
}));

describe('FirstRunGuide', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.mocked(getRecommendedOnboardingModels).mockReturnValue([
      {
        id: 'sensevoice',
        name: 'SenseVoice',
        description: 'settings.descriptions.sensevoice',
        size: '~155 MB',
        language: 'zh,en',
      },
      {
        id: 'silero-vad',
        name: 'Silero VAD',
        description: 'settings.descriptions.silero_vad',
        size: '629KB',
        language: 'zh,en',
      },
    ] as any);
    vi.mocked(downloadRecommendedOnboardingModels).mockResolvedValue({
      streamingModelPath: '/models/live',
      offlineModelPath: '/models/offline',
      vadModelPath: '/models/vad',
    });
    vi.mocked(getRecommendedOnboardingConfig).mockReturnValue({
      streamingModelPath: '/models/live',
      offlineModelPath: '/models/offline',
      vadModelPath: '/models/vad',
    });
    vi.mocked(requestMicrophonePermission).mockResolvedValue(true);
    vi.mocked(listMicrophoneDeviceOptions).mockResolvedValue([
      { label: 'settings.mic_auto', value: 'default' },
      { label: 'Desk Mic', value: 'desk-mic' },
    ]);

    useTranscriptStore.setState({
      mode: 'batch',
      config: {
        ...useTranscriptStore.getState().config,
        streamingModelPath: '',
        offlineModelPath: '',
        microphoneId: 'default',
      },
    });
    useOnboardingStore.setState({
      persistedState: { version: 1, status: 'pending' },
      currentStep: 'welcome',
      entryContext: 'startup',
      isOpen: true,
      focusStartRecordingToken: 0,
    });
  });

  it('walks through welcome, model download, and microphone setup', async () => {
    render(<FirstRunGuide />);

    expect(screen.getByText('first_run.welcome.heading')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.continue' }));
    expect(screen.getByText('first_run.models.heading')).toBeDefined();

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.download_recommended' }));
    });

    await waitFor(() => {
      expect(screen.getByText('first_run.microphone.heading')).toBeDefined();
    });

    await act(async () => {
      fireEvent.change(screen.getByLabelText('onboarding-microphone-select'), {
        target: { value: 'desk-mic' },
      });
      fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.finish' }));
    });

    expect(useTranscriptStore.getState().config.streamingModelPath).toBe('/models/live');
    expect(useTranscriptStore.getState().config.offlineModelPath).toBe('/models/offline');
    expect(useTranscriptStore.getState().config.microphoneId).toBe('desk-mic');
    expect(useTranscriptStore.getState().mode).toBe('live');
    expect(useOnboardingStore.getState().persistedState.status).toBe('completed');
    expect(useOnboardingStore.getState().isOpen).toBe(false);
  });

  it('allows deferring from the welcome step', () => {
    render(<FirstRunGuide />);

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.later' }));

    expect(useOnboardingStore.getState().persistedState.status).toBe('deferred');
    expect(useOnboardingStore.getState().isOpen).toBe(false);
  });
});
