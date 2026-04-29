import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { DiagnosticsModal } from '../DiagnosticsModal';

const mocks = vi.hoisted(() => ({
  collectSnapshot: vi.fn(),
  requestMicrophonePermission: vi.fn(),
  retryWarmup: vi.fn(),
  invoke: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      (options?.defaultValue as string | undefined) ?? key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../services/diagnosticsService', () => ({
  diagnosticsService: {
    collectSnapshot: mocks.collectSnapshot,
  },
}));

vi.mock('../../services/audioDeviceService', () => ({
  requestMicrophonePermission: mocks.requestMicrophonePermission,
}));

vi.mock('../../services/voiceTypingService', () => ({
  voiceTypingService: {
    retryWarmup: mocks.retryWarmup,
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
}));

const snapshot = {
  scannedAt: '2026-04-28T09:00:00.000Z',
  runtimeEnvironment: {
    ffmpegPath: 'C:\\app\\ffmpeg.exe',
    ffmpegExists: true,
    logDirPath: 'C:\\app\\logs',
  },
  overview: [
    {
      id: 'live-record',
      title: 'Live Record',
      description: 'Model, VAD, permission, and microphone selection for real-time capture.',
      status: 'failed' as const,
      action: {
        kind: 'open_settings' as const,
        label: 'Open Model Settings',
        settingsTab: 'models' as const,
      },
    },
  ],
  sections: [
    {
      id: 'input-capture',
      title: 'Input & Capture',
      description: 'Check permissions and device availability.',
      checks: [
        {
          id: 'microphone-permission',
          title: 'Microphone Permission',
          description: 'Microphone access has not been granted yet.',
          status: 'warning' as const,
          action: {
            kind: 'request_microphone_permission' as const,
            label: 'Request Permission',
          },
        },
      ],
    },
  ],
};

describe('DiagnosticsModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.collectSnapshot.mockResolvedValue(snapshot);
    mocks.requestMicrophonePermission.mockResolvedValue(true);
    mocks.retryWarmup.mockResolvedValue(undefined);
    mocks.invoke.mockResolvedValue(undefined);
  });

  it('loads diagnostics when opened and renders overview content', async () => {
    const { container } = render(
      <DiagnosticsModal
        isOpen={true}
        onClose={vi.fn()}
        onOpenSettingsTab={vi.fn()}
        onRunFirstRunSetup={vi.fn()}
      />,
    );

    expect(await screen.findByText('Model & Environment Diagnostics')).toBeDefined();
    expect(container.querySelector('.panel-modal-shell')).toBeTruthy();
    expect(container.querySelector('.panel-modal-header')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Close' })).toBeDefined();
    expect(screen.getByText('Live Record')).toBeDefined();
    expect(screen.getByText('Input & Capture')).toBeDefined();
    expect(mocks.collectSnapshot.mock.calls.length).toBeGreaterThan(0);
  });

  it('forwards open-settings actions to the requested settings tab', async () => {
    const onOpenSettingsTab = vi.fn();

    render(
      <DiagnosticsModal
        isOpen={true}
        onClose={vi.fn()}
        onOpenSettingsTab={onOpenSettingsTab}
        onRunFirstRunSetup={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Open Model Settings' }));

    expect(onOpenSettingsTab).toHaveBeenCalledWith('models');
  });

  it('requests microphone permission and refreshes the snapshot after the action completes', async () => {
    render(
      <DiagnosticsModal
        isOpen={true}
        onClose={vi.fn()}
        onOpenSettingsTab={vi.fn()}
        onRunFirstRunSetup={vi.fn()}
      />,
    );

    await screen.findByText('Microphone Permission');
    const callCountBeforeAction = mocks.collectSnapshot.mock.calls.length;

    fireEvent.click(await screen.findByRole('button', { name: 'Request Permission' }));

    await waitFor(() => {
      expect(mocks.requestMicrophonePermission).toHaveBeenCalledTimes(1);
      expect(mocks.collectSnapshot.mock.calls.length).toBeGreaterThan(callCountBeforeAction);
    });
  });
});
