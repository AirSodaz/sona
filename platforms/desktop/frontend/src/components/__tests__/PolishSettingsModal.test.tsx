import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useConfigStore } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import { PolishSettingsModal } from '../PolishSettingsModal';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string } & Record<string, unknown>) => {
      if (typeof options?.defaultValue === 'string') {
        return options.defaultValue.replace(/\{\{(\w+)\}\}/g, (_: string, variable: string) =>
          String(options?.[variable] ?? '')
        );
      }
      return key;
    },
  }),
}));

vi.mock('../../services/tauri/app', () => ({
  resolveEffectiveConfig: vi.fn(async (globalConfig: any) => globalConfig),
}));

describe('PolishSettingsModal', () => {
  beforeEach(() => {
    vi.restoreAllMocks();

    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        autoPolish: true,
        autoPolishFrequency: 5,
        polishPresetId: 'clean',
        polishCustomPresets: [],
      },
    });

    useProjectStore.setState({
      ...useProjectStore.getState(),
      projects: [],
      activeProjectId: null,
    });

    useTranscriptStore.setState({
      ...useTranscriptStore.getState(),
      config: useConfigStore.getState().config,
    });
  });

  it('renders modal with mode selection options', () => {
    render(<PolishSettingsModal isOpen onClose={() => undefined} />);

    expect(screen.getByText('polish.advanced_settings')).toBeDefined();
    expect(screen.getByText('Polish Mode')).toBeDefined();
    expect(screen.getByDisplayValue('5')).toBeDefined();
  });

  it('updates auto polish frequency', async () => {
    render(<PolishSettingsModal isOpen onClose={() => undefined} />);

    const input = screen.getByLabelText('Auto-Polish Frequency');
    fireEvent.change(input, { target: { value: '10' } });

    await waitFor(() => {
      expect(useConfigStore.getState().config.autoPolishFrequency).toBe(10);
    });
  });
});
