import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { FirstRunGuide } from '../FirstRunGuide';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
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

vi.mock('../../services/storageService', () => ({
  settingsStore: {
    set: vi.fn(),
    save: vi.fn(),
    get: vi.fn(),
  },
  STORE_KEY_ONBOARDING: 'sona_onboarding',
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
      batchModelPath: '/models/batch',
      vadModelPath: '/models/vad',
    });
    vi.mocked(getRecommendedOnboardingConfig).mockReturnValue({
      streamingModelPath: '/models/live',
      batchModelPath: '/models/batch',
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
        batchModelPath: '',
        microphoneId: 'default',
      },
    });
    useOnboardingStore.setState({
      persistedState: { version: 1, status: 'pending' },
      currentStep: 'microphone',
      entryContext: 'startup',
      isOpen: true,
      focusStartRecordingToken: 0,
    });
  });

  it('shows only later and continue on the microphone step', async () => {
    render(<FirstRunGuide />);

    await waitFor(() => {
      expect((screen.getByRole('button', { name: 'first_run.actions.continue' }) as HTMLButtonElement).disabled).toBe(false);
    });

    expect(screen.getByRole('button', { name: 'first_run.actions.later' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'first_run.actions.back' })).toBeNull();
  });

  it('walks through microphone and model download', async () => {
    render(<FirstRunGuide />);

    await waitFor(() => {
      expect(screen.getByText('first_run.microphone.heading')).toBeDefined();
      expect((screen.getByRole('button', { name: 'first_run.actions.continue' }) as HTMLButtonElement).disabled).toBe(false);
    });

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.continue' }));
    
    await waitFor(() => {
      expect(screen.getByText('first_run.models.heading')).toBeDefined();
    });

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.download_recommended' }));

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'first_run.actions.finish' })).toBeDefined();
      expect((screen.getByRole('button', { name: 'first_run.actions.finish' }) as HTMLButtonElement).disabled).toBe(false);
    });

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.finish' }));

    expect(useConfigStore.getState().config.streamingModelPath).toBe('/models/live');
    expect(useConfigStore.getState().config.batchModelPath).toBe('/models/batch');
    expect(useTranscriptStore.getState().mode).toBe('live');
    expect(useOnboardingStore.getState().persistedState.status).toBe('completed');
    expect(useOnboardingStore.getState().isOpen).toBe(false);
  });

  it('shows later, back, and disables both while model download is in progress', async () => {
    vi.mocked(downloadRecommendedOnboardingModels).mockImplementation(
      () => new Promise(() => {})
    );

    render(<FirstRunGuide />);

    await waitFor(() => {
      expect((screen.getByRole('button', { name: 'first_run.actions.continue' }) as HTMLButtonElement).disabled).toBe(false);
    });

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.continue' }));

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'first_run.actions.back' })).toBeDefined();
    });

    expect(screen.getByRole('button', { name: 'first_run.actions.later' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'first_run.actions.download_recommended' })).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.download_recommended' }));

    expect((screen.getByRole('button', { name: 'first_run.actions.later' }) as HTMLButtonElement).disabled)
      .toBe(true);
    expect((screen.getByRole('button', { name: 'first_run.actions.back' }) as HTMLButtonElement).disabled)
      .toBe(true);
  });

  it('can navigate back to microphone from models', async () => {
    render(<FirstRunGuide />);

    await waitFor(() => {
      expect((screen.getByRole('button', { name: 'first_run.actions.continue' }) as HTMLButtonElement).disabled).toBe(false);
    });

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.continue' }));

    await waitFor(() => {
      expect(screen.getByText('first_run.models.heading')).toBeDefined();
      expect((screen.getByRole('button', { name: 'first_run.actions.back' }) as HTMLButtonElement).disabled).toBe(false);
    });

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.back' }));

    await waitFor(() => {
      expect(screen.getByText('first_run.microphone.heading')).toBeDefined();
    });
  });

  it('allows deferring from the microphone step', async () => {
    render(<FirstRunGuide />);

    await waitFor(() => {
      expect((screen.getByRole('button', { name: 'first_run.actions.later' }) as HTMLButtonElement).disabled).toBe(false);
    });

    fireEvent.click(screen.getByRole('button', { name: 'first_run.actions.later' }));

    await waitFor(() => {
      expect(useOnboardingStore.getState().persistedState.status).toBe('deferred');
      expect(useOnboardingStore.getState().isOpen).toBe(false);
    });
  });
});
