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
    getModelCatalogSnapshot: vi.fn().mockResolvedValue({ models: {}, totalCount: 0 }),
    deleteModel: vi.fn().mockResolvedValue(true),
    getModelPath: vi.fn().mockResolvedValue('/path/to/model'),
  },
}));

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

    // Settings modal is open
    expect(screen.getByRole('dialog', { name: 'settings.title' })).toBeTruthy();
    const addBtn = await screen.findByRole(
      'button',
      { name: 'settings.speech_ai.ai_add' },
      { timeout: 5000 }
    );
    await act(async () => {
      fireEvent.click(addBtn);
    });
    expect(screen.getByRole('dialog', { name: 'settings.speech_ai.connect_title' })).toBeTruthy();

    await act(async () => {
      fireEvent.keyDown(window, { key: 'Escape' });
    });

    expect(screen.queryByRole('dialog', { name: 'settings.speech_ai.connect_title' })).toBeNull();
    expect(onSettingsClose).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog', { name: 'settings.title' })).toBeTruthy();

    await act(async () => {
      fireEvent.keyDown(window, { key: 'Escape' });
    });
    expect(onSettingsClose).not.toHaveBeenCalled();
    expect(screen.getByText('settings.speech_ai.sec_speech')).toBeTruthy();

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
    // Click "Add model provider" button
    const addBtn = await screen.findByRole(
      'button',
      { name: 'settings.speech_ai.ai_add' },
      { timeout: 5000 }
    );
    await act(async () => {
      fireEvent.click(addBtn);
    });
    const addProviderModal = screen.getByRole('dialog', {
      name: 'settings.speech_ai.connect_title',
    });
    const backdrop = addProviderModal.closest('.speech-ai-dialog-backdrop') as HTMLElement;
    expect(backdrop).toBeTruthy();

    await act(async () => {
      fireEvent.mouseDown(addProviderModal);
    });
    expect(screen.getByRole('dialog', { name: 'settings.speech_ai.connect_title' })).toBeTruthy();
    expect(onSettingsClose).not.toHaveBeenCalled();

    await act(async () => {
      fireEvent.mouseDown(backdrop);
    });
    expect(screen.queryByRole('dialog', { name: 'settings.speech_ai.connect_title' })).toBeNull();
    expect(onSettingsClose).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog', { name: 'settings.title' })).toBeTruthy();
  });
});
