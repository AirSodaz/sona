import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Settings } from '../../Settings';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
    i18n: { changeLanguage: vi.fn() },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../../services/modelService', () => ({
  PRESET_MODELS: [],
  PRESET_MODELS_MAP: new Map(),
  modelService: {
    isModelInstalled: vi.fn().mockResolvedValue(false),
    checkHardware: vi.fn().mockResolvedValue({ compatible: true }),
    getModelCatalogSnapshot: vi.fn().mockResolvedValue({
      modelsDir: '/models',
      models: [],
      sections: [],
      selectionOptions: {
        streaming: [],
        batch: [],
        speakerSegmentation: [],
        speakerEmbedding: [],
      },
      modelPathById: {},
      modelIdByNormalizedPath: {},
      pathMatchTokens: [],
      dependencyRequestsByModelId: {},
      restoreDefaults: {
        punctuationModelPath: '',
        speakerSegmentationModelPath: '',
        speakerEmbeddingModelPath: '',
        enableITN: true,
        batchVadEnabled: true,
        vadBufferSize: 5,
        maxConcurrent: 2,
      },
    }),
    resolveAsrSelectedModelIdsFromSnapshot: vi.fn().mockReturnValue({
      streaming: null,
      batch: null,
    }),
    resolveScenarioSelectedModelIdsFromSnapshot: vi.fn().mockReturnValue({}),
    deleteModel: vi.fn().mockResolvedValue(true),
    getModelPath: vi.fn().mockResolvedValue('/path/to/model'),
  },
}));

async function openAiAdvancedCatalog() {
  const advanced = await screen.findByRole('button', { name: /Advanced/i });
  await act(async () => {
    fireEvent.click(advanced);
  });
  const aiAdvanced = await screen.findByRole('button', { name: /AI advanced/i });
  await act(async () => {
    fireEvent.click(aiAdvanced);
  });
}

describe('Settings LLM Service Tab - Add Provider Modal Escape and Backdrop', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('pressing Escape while Add Provider modal is open closes only the Add Provider modal, not Settings', async () => {
    const onSettingsClose = vi.fn();

    await act(async () => {
      render(<Settings isOpen={true} onClose={onSettingsClose} initialTab="llm_service" />);
      await Promise.resolve();
    });

    // Settings modal is open (legacy llm_service redirects to Capabilities)
    expect(screen.getByRole('dialog', { name: 'settings.title' })).toBeTruthy();
    await openAiAdvancedCatalog();

    // Click "Add model provider" button
    const addBtn = await screen.findByRole(
      'button',
      { name: 'settings.llm.add_custom_provider' },
      { timeout: 5000 }
    );
    await act(async () => {
      fireEvent.click(addBtn);
    });
    expect(screen.getByRole('dialog', { name: 'settings.llm.add_custom_provider' })).toBeTruthy();

    // Press Escape
    await act(async () => {
      fireEvent.keyDown(window, { key: 'Escape' });
    });

    // Add Provider modal should be closed
    expect(screen.queryByRole('dialog', { name: 'settings.llm.add_custom_provider' })).toBeNull();

    // Settings modal should NOT be closed
    expect(onSettingsClose).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog', { name: 'settings.title' })).toBeTruthy();

    // Pressing Escape again now should close Settings
    await act(async () => {
      fireEvent.keyDown(window, { key: 'Escape' });
    });
    expect(onSettingsClose).toHaveBeenCalledTimes(1);
  });

  it('clicking the backdrop of Add Provider modal closes only the Add Provider modal, not Settings', async () => {
    const onSettingsClose = vi.fn();

    await act(async () => {
      render(<Settings isOpen={true} onClose={onSettingsClose} initialTab="llm_service" />);
      await Promise.resolve();
    });
    await openAiAdvancedCatalog();

    // Click "Add model provider" button
    const addBtn = await screen.findByRole(
      'button',
      { name: 'settings.llm.add_custom_provider' },
      { timeout: 5000 }
    );
    await act(async () => {
      fireEvent.click(addBtn);
    });
    const addProviderModal = screen.getByRole('dialog', {
      name: 'settings.llm.add_custom_provider',
    });
    const backdrop = addProviderModal.closest('.shared-modal-overlay') as HTMLElement;
    expect(backdrop).toBeTruthy();

    // Clicking inside the modal card does not close it or settings
    await act(async () => {
      fireEvent.click(addProviderModal);
    });
    expect(screen.getByRole('dialog', { name: 'settings.llm.add_custom_provider' })).toBeTruthy();
    expect(onSettingsClose).not.toHaveBeenCalled();

    // Clicking the backdrop closes Add Provider modal, leaves Settings open
    await act(async () => {
      fireEvent.click(backdrop);
    });
    expect(screen.queryByRole('dialog', { name: 'settings.llm.add_custom_provider' })).toBeNull();
    expect(onSettingsClose).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog', { name: 'settings.title' })).toBeTruthy();
  });
});
