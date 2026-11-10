import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelManagerContext } from '../../../hooks/useModelManager';
import { useConfigStore } from '../../../stores/configStore';
import { setTestConfig } from '../../../test-utils/configTestUtils';
import { SettingsSpeechAiTab } from './SettingsSpeechAiTab';
import { SENSEVOICE_STANDARD_ID } from './speechAiLogic';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en', resolvedLanguage: 'en' },
  }),
  initReactI18next: { type: '3rdParty', init: () => undefined },
}));

vi.mock('../../../services/tauri/llm', () => ({
  generateLlmText: vi.fn(() => new Promise(() => undefined)),
  listLlmModels: vi.fn(async () => []),
}));

vi.mock('../../../services/cudaAddonService', () => ({
  cudaAddonService: {
    getStatus: vi.fn(async () => null),
    downloadAndInstall: vi.fn(),
  },
}));

function renderTab() {
  const managerValue = {
    deletingId: null,
    downloads: {},
    installedModels: new Set<string>(),
    modelCatalog: {
      models: [],
      modelPathById: {},
      sections: [],
      selectionOptions: {
        streaming: [],
        batch: [],
        speakerSegmentation: [],
        speakerEmbedding: [],
        alignment: [],
      },
    },
    selectedModelIds: {},
    catalogLoadState: 'ready',
    catalogLoadError: null,
    handleDelete: vi.fn(),
    handleDownload: vi.fn(async () => undefined),
    handleCancelDownload: vi.fn(),
    handleDiscardDownload: vi.fn(),
    restoreDefaultModelSettings: vi.fn(),
  } as never;

  return render(
    <ModelManagerContext.Provider value={managerValue}>
      <SettingsSpeechAiTab />
    </ModelManagerContext.Provider>
  );
}

describe('SettingsSpeechAiTab v2', () => {
  beforeEach(() => {
    setTestConfig({
      liveSpeakerDiarizationEnabled: false,
      batchSpeakerDiarizationEnabled: false,
      speechModesLinked: true,
      asr: {
        selections: {
          live: {
            engine: 'local',
            mode: 'streaming',
            modelId: SENSEVOICE_STANDARD_ID,
            modelPath: '/models/sensevoice',
          },
        },
      },
    });
  });

  it('keeps one primary action and the flattened speech rows', () => {
    const { container } = renderTab();
    expect(container.querySelectorAll('.btn-primary')).toHaveLength(1);
    expect(screen.getByText('settings.speech_ai.setup_cta')).toBeTruthy();
    expect(screen.queryByText('settings.speech_ai.more_options')).toBeNull();
    expect(screen.getByText('settings.speech_ai.ctc_live')).toBeTruthy();
    expect(screen.getByText('settings.speech_ai.ctc_batch')).toBeTruthy();
    expect(screen.queryByText('settings.speech_ai.embedding')).toBeNull();

    fireEvent.click(screen.getByRole('switch', { name: 'settings.speech_ai.speakers_live' }));
    expect(useConfigStore.getState().config.liveSpeakerDiarizationEnabled).toBe(true);
    expect(screen.getByText('settings.speech_ai.embedding')).toBeTruthy();
    expect(screen.getByText('settings.speech_ai.count')).toBeTruthy();
    expect(container.querySelectorAll('.btn-primary')).toHaveLength(1);
  });

  it('changes only the recording language from the recording model page', () => {
    renderTab();
    fireEvent.click(
      screen.getByRole('button', {
        name: /settings\.speech_ai\.live,/,
      })
    );
    fireEvent.click(document.getElementById('settings-speech-language-dropdown') as HTMLElement);
    fireEvent.click(screen.getByRole('option', { name: 'English' }));
    const config = useConfigStore.getState().config;
    expect(config.liveLanguage).toBe('en');
    expect(config.batchLanguage).not.toBe('en');
  });
});
