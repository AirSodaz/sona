import { render, screen } from '@testing-library/react';
import type React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsNavigationProvider } from '../SettingsNavigationContext';

const mockConfig = {
  streamingModelPath: '',
  asr: undefined as
    | {
        selections: {
          live: { engine: string; modelPath: string; modelId: string };
        };
      }
    | undefined,
  llmSettings: undefined,
};

const modelManagerState = {
  selectedModelIds: { streaming: null as string | null, batch: null as string | null },
  downloads: {} as Record<string, unknown>,
  modelCatalog: {
    sections: [] as unknown[],
    models: [] as Array<{ id: string; name: string; installPath?: string }>,
    modelPathById: {} as Record<string, string>,
    dependencyRequestsByModelId: {},
  },
  catalogLoadState: 'ready',
  handleDownload: vi.fn(),
  handleLoad: vi.fn(),
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string; name?: string }) => {
      if (options?.defaultValue && options.name) {
        return options.defaultValue.replace('{{name}}', options.name);
      }
      return options?.defaultValue ?? key;
    },
  }),
  initReactI18next: { type: '3rdParty', init: vi.fn() },
}));

vi.mock('../../../hooks/useModelManager', () => ({
  useModelManagerContext: () => modelManagerState,
  ModelManagerContext: { Provider: ({ children }: { children: React.ReactNode }) => children },
}));

vi.mock('../../../stores/configStore', () => ({
  useModelConfig: () => mockConfig,
  useLlmAssistantConfig: () => mockConfig,
  useSetConfig: () => vi.fn(),
  useConfigStore: (selector: (state: { config: typeof mockConfig }) => unknown) =>
    selector({ config: mockConfig }),
}));

vi.mock('../../../stores/dialogStore', () => ({
  useDialogStore: (selector: (state: { showError: () => void }) => unknown) =>
    selector({ showError: vi.fn() }),
}));

vi.mock('../SettingsModelsTab', () => ({
  SettingsModelsTab: () => <div data-testid="embedded-models">models</div>,
}));

vi.mock('../SettingsLLMServiceTab', () => ({
  SettingsLLMServiceTab: () => <div data-testid="embedded-llm">llm</div>,
}));

import { SettingsCapabilitiesTab } from '../SettingsCapabilitiesTab';

describe('SettingsCapabilitiesTab', () => {
  beforeEach(() => {
    mockConfig.streamingModelPath = '';
    mockConfig.asr = undefined;
    mockConfig.llmSettings = undefined;
    modelManagerState.selectedModelIds = { streaming: null, batch: null };
    modelManagerState.modelCatalog = {
      sections: [],
      models: [],
      modelPathById: {},
      dependencyRequestsByModelId: {},
    };
  });

  it('renders the three capability cards and advanced escape hatch', () => {
    render(
      <SettingsNavigationProvider value={{ activeTab: 'capabilities', navigateToTab: vi.fn() }}>
        <SettingsCapabilitiesTab />
      </SettingsNavigationProvider>
    );

    expect(screen.getByText('Capabilities')).toBeTruthy();
    expect(screen.getByText('Speech Recognition')).toBeTruthy();
    expect(screen.getByText('AI Writing')).toBeTruthy();
    expect(screen.getByText('Translation')).toBeTruthy();
    expect(screen.getByText('Install recommended')).toBeTruthy();
    expect(screen.getByText('Connect AI Writing')).toBeTruthy();
    expect(screen.getByText('Advanced')).toBeTruthy();
    expect(screen.getByText(/Engine:/)).toBeTruthy();
    expect(screen.getByText('Change engine')).toBeTruthy();
  });

  it('shows a human catalog name on Speech Ready instead of a raw path', () => {
    mockConfig.streamingModelPath = '/models/sensevoice-small.onnx';
    modelManagerState.selectedModelIds = { streaming: 'sensevoice-small', batch: null };
    modelManagerState.modelCatalog = {
      sections: [],
      models: [{ id: 'sensevoice-small', name: 'SenseVoice Small' }],
      modelPathById: { 'sensevoice-small': '/models/sensevoice-small.onnx' },
      dependencyRequestsByModelId: {},
    };

    render(
      <SettingsNavigationProvider value={{ activeTab: 'capabilities', navigateToTab: vi.fn() }}>
        <SettingsCapabilitiesTab />
      </SettingsNavigationProvider>
    );

    expect(screen.getByText('Ready · SenseVoice Small')).toBeTruthy();
    expect(screen.getByText('Change model')).toBeTruthy();
    expect(screen.queryByText('Install recommended')).toBeNull();
  });
});
