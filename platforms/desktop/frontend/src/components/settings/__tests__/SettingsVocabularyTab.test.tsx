import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAutomationStore } from '../../../stores/automationStore';
import { useConfigStore } from '../../../stores/configStore';
import { useProjectStore } from '../../../stores/projectStore';
import { SettingsVocabularyTab } from '../SettingsVocabularyTab';

vi.mock('../../../services/automation/automationRepository', () => ({
  loadAutomationRepositoryState: vi.fn(),
  persistAutomationProcessedEntries: vi.fn().mockResolvedValue(undefined),
  persistAutomationProfiles: vi.fn().mockResolvedValue(undefined),
  persistAutomationRepositoryState: vi.fn().mockResolvedValue(undefined),
  persistAutomationRules: vi.fn().mockResolvedValue(undefined),
  validateAutomationRuleActivation: vi.fn().mockResolvedValue(undefined),
}));

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
        polishPresetId: 'clean',
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
          icon: '',
          createdAt: 1,
          updatedAt: 1,
        },
      ],
      activeProjectId: null,
    });
    useAutomationStore.setState({
      profiles: [
        {
          id: 'profile-1',
          name: 'Team profile',
          translationLanguage: 'en',
          polishPresetId: 'custom-team',
          summaryTemplateId: 'summary-team',
          enabledTextReplacementSetIds: ['text-1'],
          enabledHotwordSetIds: ['hot-1'],
          enabledPolishKeywordSetIds: ['kw-1'],
          enabledSpeakerProfileIds: ['speaker-1'],
          createdAt: 1,
          updatedAt: 1,
        },
      ],
    });
  });

  it('renders secondary category tabs and switches between recognition and prompts sections', () => {
    render(<SettingsVocabularyTab />);

    screen.getByRole('tab', { name: 'Unified Dictionary' });
    screen.getByRole('tab', { name: 'AI Prompts & Templates' });
    screen.getByRole('tab', { name: 'Speaker Profiles' });
    expect(screen.getAllByText('Unified Dictionary').length).toBeGreaterThanOrEqual(1);
    expect(screen.queryByText('Polish Keywords')).toBeNull();

    fireEvent.click(screen.getByRole('tab', { name: 'AI Prompts & Templates' }));

    screen.getByText('Built-in Presets');
    screen.getByText('Built-in Summary Templates');
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
    render(<SettingsVocabularyTab initialSubTab="prompts" />);

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
      expect(useAutomationStore.getState().profiles[0].summaryTemplateId).toBe('general');
    });
  });

  it('updates and deletes custom presets while resetting affected automation profiles', async () => {
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        polishPresetId: 'custom-team',
        polishCustomPresets: [{ id: 'custom-team', name: 'Team', context: 'Initial context' }],
      },
    });

    render(<SettingsVocabularyTab initialSubTab="prompts" />);

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
      expect(useConfigStore.getState().config.polishPresetId).toBe('clean');
      expect(useAutomationStore.getState().profiles[0].polishPresetId).toBe('clean');
    });
  });

  it('deletes speaker profiles and removes their automation profile references', async () => {
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        speakerProfiles: [{ id: 'speaker-1', name: 'Alice', enabled: true, samples: [] }],
      },
    });
    render(<SettingsVocabularyTab initialSubTab="speakers" />);

    fireEvent.click(screen.getByRole('button', { name: 'Delete Alice' }));

    await waitFor(() => {
      expect(useConfigStore.getState().config.speakerProfiles).toEqual([]);
      expect(useAutomationStore.getState().profiles[0].enabledSpeakerProfileIds).toEqual([]);
    });
  });

  it('shows readiness guidance for speaker profiles based on usable samples', () => {
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        speakerProfiles: [
          {
            id: 'speaker-ready',
            name: 'Alice',
            enabled: true,
            samples: [
              {
                id: 'sample-1',
                filePath: '/alice-1.wav',
                sourceName: 'Alice 1',
                durationSeconds: 10,
              },
              {
                id: 'sample-2',
                filePath: '/alice-2.wav',
                sourceName: 'Alice 2',
                durationSeconds: 11,
              },
            ],
          },
          {
            id: 'speaker-limited',
            name: 'Bob',
            enabled: true,
            samples: [
              { id: 'sample-3', filePath: '/bob-1.wav', sourceName: 'Bob 1', durationSeconds: 8.5 },
            ],
          },
          {
            id: 'speaker-not-ready',
            name: 'Carol',
            enabled: true,
            samples: [
              {
                id: 'sample-4',
                filePath: '/carol-1.wav',
                sourceName: 'Carol 1',
                durationSeconds: 3.5,
              },
            ],
          },
        ],
      },
    });

    render(<SettingsVocabularyTab initialSubTab="speakers" />);

    screen.getByText('Ready for automatic matching');
    screen.getByText(
      'Can appear as a suggestion, but needs more usable samples before automatic matching.'
    );
    screen.getByText('Needs more usable samples before it can participate in speaker recognition.');
  });

  it('supports keyboard navigation across sub-tabs', () => {
    render(<SettingsVocabularyTab />);
    const tablist = screen.getByRole('tablist', { name: 'Vocabulary categories' });

    fireEvent.keyDown(tablist, { key: 'ArrowRight' });
    expect(
      screen.getByRole('tab', { name: 'AI Prompts & Templates' }).getAttribute('aria-selected')
    ).toBe('true');
    screen.getByText('Built-in Presets');

    fireEvent.keyDown(tablist, { key: 'ArrowRight' });
    expect(
      screen.getByRole('tab', { name: 'Speaker Profiles' }).getAttribute('aria-selected')
    ).toBe('true');

    fireEvent.keyDown(tablist, { key: 'ArrowLeft' });
    expect(
      screen.getByRole('tab', { name: 'AI Prompts & Templates' }).getAttribute('aria-selected')
    ).toBe('true');
  });
});
