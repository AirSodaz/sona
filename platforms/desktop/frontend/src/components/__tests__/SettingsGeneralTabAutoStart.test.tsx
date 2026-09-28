import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';
import { SettingsGeneralTab } from '../settings/SettingsGeneralTab';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => {
      const translations: Record<string, string> = {
        'settings.auto_start': 'Launch on startup',
        'settings.auto_start_hint': 'Automatically launch on startup',
        'settings.minimize_to_tray': 'Minimize to tray on exit',
        'settings.minimize_to_tray_hint': 'Minimize to tray on exit hint',
        'settings.silent_start': 'Silent launch',
        'settings.silent_start_hint': 'Start minimized to tray on startup',
        'settings.auto_check_updates': 'Automatically check for updates',
        'settings.general': 'General',
        'settings.general_description': 'General settings',
        'settings.general_title': 'General',
        'settings.language': 'Language',
        'settings.language_hint': 'Language hint',
        'settings.theme': 'Theme',
        'settings.log_level': 'Log Level',
        'settings.log_level_hint': 'Log Level hint',
      };
      return translations[key] ?? options?.defaultValue ?? key;
    },
  }),
}));

vi.mock('../../stores/dialogStore', () => ({
  useDialogStore: (selector: (state: unknown) => unknown) =>
    selector({
      showError: vi.fn(),
      confirm: vi.fn().mockResolvedValue(false),
    }),
}));

vi.mock('../../stores/transcriptRuntimeStore', () => ({
  useTranscriptRuntimeStore: (selector: (state: unknown) => unknown) =>
    selector({
      isRecording: false,
    }),
}));
describe('SettingsGeneralTab AutoStart & SilentStart', () => {
  beforeEach(() => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        autoStart: false,
        silentStart: false,
        minimizeToTrayOnExit: true,
      },
    });
  });

  it('renders auto_start switch with default false and hides silent_start', () => {
    render(<SettingsGeneralTab />);

    expect(screen.getByText('Launch on startup')).toBeTruthy();
    expect(screen.getByText('Minimize to tray on exit')).toBeTruthy();
    // silent_start should NOT be rendered when autoStart is false
    expect(screen.queryByText('Silent launch')).toBeNull();
  });

  it('does not show silent_start if autoStart is true but minimizeToTrayOnExit is false', () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        autoStart: true,
        silentStart: false,
        minimizeToTrayOnExit: false,
      },
    });

    render(<SettingsGeneralTab />);
    expect(screen.queryByText('Silent launch')).toBeNull();
  });

  it('shows silent_start when both autoStart and minimizeToTrayOnExit are true', () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        autoStart: true,
        silentStart: false,
        minimizeToTrayOnExit: true,
      },
    });

    render(<SettingsGeneralTab />);
    expect(screen.getByText('Silent launch')).toBeTruthy();
  });

  it('updates autoStart and resets silentStart when autoStart is toggled off', () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        autoStart: true,
        silentStart: true,
        minimizeToTrayOnExit: true,
      },
    });

    render(<SettingsGeneralTab />);

    // Find the auto_start switch
    const autoStartItem = screen.getByText('Launch on startup').closest('.settings-item-container');
    const autoStartSwitch = autoStartItem?.querySelector('[role="switch"]');
    expect(autoStartSwitch).toBeTruthy();

    fireEvent.click(autoStartSwitch!);

    const state = useConfigStore.getState().config;
    expect(state.autoStart).toBe(false);
    expect(state.silentStart).toBe(false);
  });

  it('resets silentStart when minimizeToTrayOnExit is toggled off', () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        autoStart: true,
        silentStart: true,
        minimizeToTrayOnExit: true,
      },
    });

    render(<SettingsGeneralTab />);

    const trayItem = screen
      .getByText('Minimize to tray on exit')
      .closest('.settings-item-container');
    const traySwitch = trayItem?.querySelector('[role="switch"]');
    expect(traySwitch).toBeTruthy();

    fireEvent.click(traySwitch!);

    const state = useConfigStore.getState().config;
    expect(state.minimizeToTrayOnExit).toBe(false);
    expect(state.silentStart).toBe(false);
  });

  it('toggles silentStart when switch is clicked', () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        autoStart: true,
        silentStart: false,
        minimizeToTrayOnExit: true,
      },
    });

    render(<SettingsGeneralTab />);

    const silentItem = screen.getByText('Silent launch').closest('.settings-item-container');
    const silentSwitch = silentItem?.querySelector('[role="switch"]');
    expect(silentSwitch).toBeTruthy();

    fireEvent.click(silentSwitch!);

    const state = useConfigStore.getState().config;
    expect(state.silentStart).toBe(true);
  });
});
