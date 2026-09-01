import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
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
    });

    // Settings modal is open
    expect(screen.getByRole('dialog', { name: 'settings.title' })).toBeTruthy();
    // Click "Add model provider" button
    const addBtn = await screen.findByRole('button', { name: 'settings.llm.add_custom_provider' });
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

    let container!: HTMLElement;
    await act(async () => {
      ({ container } = render(<Settings isOpen={true} onClose={onSettingsClose} initialTab="llm_service" />));
    });
    // Click "Add model provider" button
    const addBtn = await screen.findByRole('button', { name: 'settings.llm.add_custom_provider' });
    await act(async () => {
      fireEvent.click(addBtn);
    });
    const addProviderModal = screen.getByRole('dialog', { name: 'settings.llm.add_custom_provider' });
    const backdrop = container.querySelector('.provider-modal-backdrop') as HTMLElement;
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
