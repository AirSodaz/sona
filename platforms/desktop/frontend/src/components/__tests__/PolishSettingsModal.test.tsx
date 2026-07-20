import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { PolishSettingsModal } from '../PolishSettingsModal';
import { useConfigStore } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string } & Record<string, unknown>) => {
      if (typeof options?.defaultValue === 'string') {
        return options.defaultValue.replace(/\{\{(\w+)\}\}/g, (_: string, variable: string) => String(options?.[variable] ?? ''));
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
        polishPresetId: 'general',
        polishCustomPresets: [],
        polishKeywordSets: [
          { id: 'kw-1', name: 'Brand Terms', enabled: true, keywords: 'Sona' },
          { id: 'kw-2', name: 'Style Guide', enabled: false, keywords: 'Sentence case' },
        ],
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

  it('updates global keyword enablement when no project is active', async () => {
    render(<PolishSettingsModal isOpen onClose={() => undefined} />);

    fireEvent.click(screen.getByRole('checkbox', { name: 'Style Guide' }));

    await waitFor(() => {
      expect(useConfigStore.getState().config.polishKeywordSets).toEqual([
        expect.objectContaining({ id: 'kw-1', enabled: true }),
        expect.objectContaining({ id: 'kw-2', enabled: true }),
      ]);
    });
  });

  it('updates global keyword enablement when a Tag is active', async () => {
    const project = {
      id: 'project-1',
      name: 'Alpha',
      description: '',
      icon: '',
      createdAt: 1,
      updatedAt: 1,
    };

    useProjectStore.setState({
      ...useProjectStore.getState(),
      projects: [project],
      activeProjectId: 'project-1',
    });
    render(<PolishSettingsModal isOpen onClose={() => undefined} />);

    fireEvent.click(screen.getByRole('checkbox', { name: 'Style Guide' }));

    await waitFor(() => {
      expect(useConfigStore.getState().config.polishKeywordSets).toEqual([
        expect.objectContaining({ id: 'kw-1', enabled: true }),
        expect.objectContaining({ id: 'kw-2', enabled: true }),
      ]);
    });
  });
});
