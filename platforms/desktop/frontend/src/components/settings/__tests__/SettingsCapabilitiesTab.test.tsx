import { render, screen } from '@testing-library/react';
import type React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsNavigationProvider } from '../SettingsNavigationContext';

const mockConfig = {
  streamingModelPath: '',
  asr: undefined,
  llmSettings: undefined,
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
  }),
  initReactI18next: { type: '3rdParty', init: vi.fn() },
}));

vi.mock('../../../hooks/useModelManager', () => ({
  useModelManagerContext: () => ({
    selectedModelIds: { streaming: null, batch: null },
    downloads: {},
    modelCatalog: { sections: [], models: [], modelPathById: {}, dependencyRequestsByModelId: {} },
    catalogLoadState: 'ready',
    handleDownload: vi.fn(),
    handleLoad: vi.fn(),
  }),
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
  });

  it('renders the three capability cards and advanced escape hatch', () => {
    render(
      <SettingsNavigationProvider
        value={{ activeTab: 'capabilities', navigateToTab: vi.fn() }}
      >
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
  });
});
