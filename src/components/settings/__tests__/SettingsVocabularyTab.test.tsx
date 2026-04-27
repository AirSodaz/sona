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
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
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
          summaryTemplateId: 'general',
          translationLanguage: 'en',
          polishPresetId: 'general',
          exportFileNamePrefix: '',
          enabledTextReplacementSetIds: [],
          enabledHotwordSetIds: [],
          enabledPolishKeywordSetIds: [],
          enabledSpeakerProfileIds: [],
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
        summaryTemplateId: 'general',
        summaryCustomTemplates: [],
        polishPresetId: 'general',
        polishCustomPresets: [],
        polishKeywordSets: [],
        speakerProfiles: [],
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
            summaryTemplateId: 'general',
            translationLanguage: 'en',
            polishPresetId: 'custom-team',
            exportFileNamePrefix: '',
            enabledTextReplacementSetIds: [],
            enabledHotwordSetIds: [],
            enabledPolishKeywordSetIds: ['kw-1'],
            enabledSpeakerProfileIds: [],
          },
        },
      ],
      activeProjectId: null,
    });
  });

  it('renders text replacement, hotwords, polish keywords, context presets, and summary templates while letting users add a custom preset', () => {
    render(<SettingsVocabularyTab />);

    expect(screen.getByText('Text Replacement')).toBeDefined();
    expect(screen.getByText('Hotwords')).toBeDefined();
    expect(screen.getByText('Polish Keywords')).toBeDefined();
    expect(screen.getByText('Built-in Presets')).toBeDefined();
    expect(screen.getByText('Built-in Summary Templates')).toBeDefined();
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

  it('adds and deletes custom summary templates while resetting affected defaults', async () => {
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        summaryTemplateId: 'summary-team',
        summaryCustomTemplates: [
          { id: 'summary-team', name: 'Team Summary', instructions: '1. Status\n2. Risks' },
        ],
      },
    });
    useProjectStore.setState({
      ...useProjectStore.getState(),
      projects: [
        {
          ...useProjectStore.getState().projects[0],
          defaults: {
            ...useProjectStore.getState().projects[0].defaults,
            summaryTemplateId: 'summary-team',
          },
        },
      ],
    });

    render(<SettingsVocabularyTab />);

    fireEvent.change(screen.getByDisplayValue('Team Summary'), {
      target: { value: 'Ops Summary' },
    });
    fireEvent.change(screen.getByRole('textbox', { name: 'Ops Summary' }), {
      target: { value: '1. Overview\n2. Follow-up' },
    });

    expect(useConfigStore.getState().config.summaryCustomTemplates).toEqual([
      expect.objectContaining({
        id: 'summary-team',
        name: 'Ops Summary',
        instructions: '1. Overview\n2. Follow-up',
      }),
    ]);

    fireEvent.click(screen.getByRole('button', { name: 'Delete Ops Summary' }));

    await waitFor(() => {
      expect(useConfigStore.getState().config.summaryCustomTemplates).toEqual([]);
      expect(useConfigStore.getState().config.summaryTemplateId).toBe('general');
      expect(useProjectStore.getState().projects[0].defaults.summaryTemplateId).toBe('general');
    });
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

  it('deletes global rule sets and removes their project-level references', async () => {
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        textReplacementSets: [
          { id: 'text-1', name: 'Text Set', enabled: true, ignoreCase: false, rules: [] },
        ],
        hotwordSets: [
          { id: 'hot-1', name: 'Hot Set', enabled: true, rules: [] },
        ],
        polishKeywordSets: [
          { id: 'kw-1', name: 'Brand Terms', enabled: true, keywords: 'Sona' },
        ],
      },
    });
    useProjectStore.setState({
      ...useProjectStore.getState(),
      projects: [
        {
          ...useProjectStore.getState().projects[0],
          defaults: {
            ...useProjectStore.getState().projects[0].defaults,
            enabledTextReplacementSetIds: ['text-1'],
            enabledHotwordSetIds: ['hot-1'],
            enabledPolishKeywordSetIds: ['kw-1'],
          },
        },
      ],
    });

    render(<SettingsVocabularyTab />);

    fireEvent.click(screen.getByRole('button', { name: 'Delete Text Set' }));
    await waitFor(() => {
      expect(useConfigStore.getState().config.textReplacementSets).toEqual([]);
      expect(useProjectStore.getState().projects[0].defaults.enabledTextReplacementSetIds).toEqual([]);
    });

    fireEvent.click(screen.getByRole('button', { name: 'Delete Hot Set' }));
    await waitFor(() => {
      expect(useConfigStore.getState().config.hotwordSets).toEqual([]);
      expect(useProjectStore.getState().projects[0].defaults.enabledHotwordSetIds).toEqual([]);
    });

    fireEvent.click(screen.getByRole('button', { name: 'Delete Brand Terms' }));
    await waitFor(() => {
      expect(useConfigStore.getState().config.polishKeywordSets).toEqual([]);
      expect(useProjectStore.getState().projects[0].defaults.enabledPolishKeywordSetIds).toEqual([]);
    });
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

  it('deletes speaker profiles and removes their project-level references', async () => {
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        speakerProfiles: [
          { id: 'speaker-1', name: 'Alice', enabled: true, samples: [] },
        ],
      },
    });
    useProjectStore.setState({
      ...useProjectStore.getState(),
      projects: [
        {
          ...useProjectStore.getState().projects[0],
          defaults: {
            ...useProjectStore.getState().projects[0].defaults,
            enabledSpeakerProfileIds: ['speaker-1'],
          },
        },
      ],
    });

    render(<SettingsVocabularyTab />);

    fireEvent.click(screen.getByRole('button', { name: 'Delete Alice' }));

    await waitFor(() => {
      expect(useConfigStore.getState().config.speakerProfiles).toEqual([]);
      expect(useProjectStore.getState().projects[0].defaults.enabledSpeakerProfileIds).toEqual([]);
    });
  });
});
