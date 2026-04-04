import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { FirstRunGuide } from '../FirstRunGuide';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { useConfigStore } from '../../stores/configStore';
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
    });
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
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

  it('shows only later and continue on the welcome step', () => {
    render(<FirstRunGuide />);

    expect(screen.getByRole('button', { name: 'first_run.actions.later' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'first_run.actions.continue' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'first_run.actions.back' })).toBeNull();
  });

  it('walks through welcome, model download, and microphone setup', async () => {
    render(<FirstRunGuide />);

    expect(screen.getByText('first_run.welcome.heading')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.continue' }));
    expect(screen.getByText('first_run.models.heading')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.download_recommended' }));

    await waitFor(() => {
      expect(screen.getByText('first_run.microphone.heading')).toBeDefined();
    });

    await waitFor(() => {
      expect((screen.getByRole('button', { name: 'first_run.actions.finish' }) as HTMLButtonElement).disabled).toBe(false);
    });

    await act(async () => {
      fireEvent.change(screen.getByLabelText('onboarding-microphone-select'), {
        target: { value: 'desk-mic' },
      });
    });

    await waitFor(() => {
      expect((screen.getByLabelText('onboarding-microphone-select') as HTMLSelectElement).value).toBe('desk-mic');
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.finish' }));
    });

    expect(useConfigStore.getState().config.streamingModelPath).toBe('/models/live');
    expect(useConfigStore.getState().config.offlineModelPath).toBe('/models/offline');
    expect(useConfigStore.getState().config.microphoneId).toBe('desk-mic');
    expect(useTranscriptStore.getState().mode).toBe('live');
    expect(useOnboardingStore.getState().persistedState.status).toBe('completed');
    expect(useOnboardingStore.getState().isOpen).toBe(false);
  });

  it('shows later, back, and disables both while model download is in progress', () => {
    vi.mocked(downloadRecommendedOnboardingModels).mockImplementation(
      () => new Promise(() => {})
    );

    render(<FirstRunGuide />);

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.continue' }));

    expect(screen.getByRole('button', { name: 'first_run.actions.later' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'first_run.actions.back' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'first_run.actions.download_recommended' })).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.download_recommended' }));

    expect((screen.getByRole('button', { name: 'first_run.actions.later' }) as HTMLButtonElement).disabled)
      .toBe(true);
    expect((screen.getByRole('button', { name: 'first_run.actions.back' }) as HTMLButtonElement).disabled)
      .toBe(true);
  });

  it('shows later, back, and finish on the microphone step, and can navigate back', async () => {
    useTranscriptStore.setState({
      mode: 'batch',
    });
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        streamingModelPath: '/models/live',
        offlineModelPath: '/models/offline',
        microphoneId: 'default',
      },
    });
    useOnboardingStore.setState({
      persistedState: { version: 1, status: 'deferred' },
      currentStep: 'microphone',
      entryContext: 'startup',
      isOpen: true,
      focusStartRecordingToken: 0,
    });

    render(<FirstRunGuide />);

    await waitFor(() => {
      expect(screen.getByText('first_run.microphone.heading')).toBeDefined();
    });

    expect(screen.getByRole('button', { name: 'first_run.actions.later' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'first_run.actions.back' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'first_run.actions.finish' })).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.back' }));

    expect(screen.getByText('first_run.models.heading')).toBeDefined();
  });

  it('allows deferring from the welcome step', () => {
    render(<FirstRunGuide />);

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.later' }));

    expect(useOnboardingStore.getState().persistedState.status).toBe('deferred');
    expect(useOnboardingStore.getState().isOpen).toBe(false);
  });
});
