import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { SettingsVocabularyTab } from '../SettingsVocabularyTab';
import { useConfigStore } from '../../../stores/configStore';
import { useProjectStore } from '../../../stores/projectStore';

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

vi.mock('../../../services/projectService', () => ({
  projectService: {
    getAll: vi.fn(),
    getActiveProjectId: vi.fn(),
    create: vi.fn(),
    update: vi.fn().mockImplementation(async (id: string, updates: any) => ({
      id,
      name: updates.name || 'Alpha',
      description: updates.description || '',
      createdAt: 1,
      updatedAt: 2,
      defaults: {
        summaryTemplate: 'general',
        translationLanguage: 'en',
        polishPresetId: 'general',
        exportFileNamePrefix: '',
        enabledTextReplacementSetIds: [],
        enabledHotwordSetIds: [],
        ...(updates.defaults || {}),
      },
    })),
    delete: vi.fn(),
    setActiveProjectId: vi.fn(),
    saveAll: vi.fn(),
  },
}));

vi.mock('../../../services/historyService', () => ({
  historyService: {
    updateProjectAssignments: vi.fn(),
    updateProjectAssignmentsByCurrentProject: vi.fn(),
  },
}));

describe('SettingsVocabularyTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        polishPresetId: 'general',
        polishCustomPresets: [],
        polishKeywordSets: [],
      },
    });

    useProjectStore.setState({
      ...useProjectStore.getState(),
      projects: [
        {
          id: 'project-1',
          name: 'Alpha',
          description: '',
          createdAt: 1,
          updatedAt: 1,
          defaults: {
            summaryTemplate: 'general',
            translationLanguage: 'en',
            polishPresetId: 'custom-team',
            exportFileNamePrefix: '',
            enabledTextReplacementSetIds: [],
            enabledHotwordSetIds: [],
          },
        },
      ],
      activeProjectId: null,
    });
  });

  it('renders text replacement, hotwords, polish keywords, and context presets while letting users add a custom preset', () => {
    render(<SettingsVocabularyTab />);

    expect(screen.getByText('Text Replacement')).toBeDefined();
    expect(screen.getByText('Hotwords')).toBeDefined();
    expect(screen.getByText('Polish Keywords')).toBeDefined();
    expect(screen.getByText('Built-in Presets')).toBeDefined();
    expect(screen.getAllByText('General').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Meeting').length).toBeGreaterThan(0);

    fireEvent.change(screen.getByPlaceholderText('Preset name'), {
      target: { value: 'Team Notes' },
    });
    fireEvent.change(screen.getByPlaceholderText('Enter preset context...'), {
      target: { value: 'Focus on roadmap terms.' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add Preset' }));

    expect(useConfigStore.getState().config.polishCustomPresets).toEqual([
      expect.objectContaining({
        name: 'Team Notes',
        context: 'Focus on roadmap terms.',
      }),
    ]);
  });

  it('adds and updates polish keyword sets', () => {
    render(<SettingsVocabularyTab />);

    fireEvent.change(screen.getByPlaceholderText('e.g. Brand Terms'), {
      target: { value: 'Brand Terms' },
    });
    fireEvent.click(screen.getAllByRole('button', { name: 'Add Set' })[2]);

    expect(useConfigStore.getState().config.polishKeywordSets).toEqual([
      expect.objectContaining({
        name: 'Brand Terms',
        enabled: true,
        keywords: '',
      }),
    ]);

    fireEvent.change(screen.getByPlaceholderText('e.g. Product names, terminology, preferred spellings'), {
      target: { value: 'Sona\nSherpa-onnx' },
    });

    expect(useConfigStore.getState().config.polishKeywordSets).toEqual([
      expect.objectContaining({
        name: 'Brand Terms',
        keywords: 'Sona\nSherpa-onnx',
      }),
    ]);
  });

  it('updates and deletes custom presets while resetting affected project defaults', async () => {
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        polishPresetId: 'custom-team',
        polishCustomPresets: [
          { id: 'custom-team', name: 'Team', context: 'Initial context' },
        ],
      },
    });

    render(<SettingsVocabularyTab />);

    fireEvent.change(screen.getByDisplayValue('Team'), {
      target: { value: 'Product Team' },
    });
    fireEvent.change(screen.getByDisplayValue('Initial context'), {
      target: { value: 'Use product terminology.' },
    });

    expect(useConfigStore.getState().config.polishCustomPresets).toEqual([
      expect.objectContaining({
        id: 'custom-team',
        name: 'Product Team',
        context: 'Use product terminology.',
      }),
    ]);

    fireEvent.click(screen.getByRole('button', { name: 'Delete Product Team' }));

    await waitFor(() => {
      expect(useConfigStore.getState().config.polishCustomPresets).toEqual([]);
      expect(useConfigStore.getState().config.polishPresetId).toBe('general');
      expect(useProjectStore.getState().projects[0].defaults.polishPresetId).toBe('general');
    });
  });
});
