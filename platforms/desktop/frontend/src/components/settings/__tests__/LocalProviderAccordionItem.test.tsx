import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createLlmSettings } from '../../../services/llm/state';
import { importLocalLlmFile, listLocalLlmCards } from '../../../services/tauri/llm';
import { openDialog } from '../../../services/tauri/platform/dialog';
import type { LlmAssistantConfig } from '../../../types/config';
import { LocalProviderAccordionItem } from '../llm/LocalProviderAccordionItem';

vi.mock('../../../services/tauri/llm', () => ({
  listLocalLlmCards: vi.fn().mockResolvedValue({ modelsDir: '/test/models', cards: [] }),
  generateLlmText: vi.fn().mockResolvedValue('test response'),
  importLocalLlmFile: vi.fn().mockResolvedValue('/test/models/custom-model.gguf'),
}));

vi.mock('../../../services/tauri/app', () => ({
  downloadPresetModel: vi.fn(),
  cancelDownload: vi.fn(),
  deletePresetModel: vi.fn(),
}));

vi.mock('../../../services/tauri/platform/dialog', () => ({
  openDialog: vi.fn(),
}));

vi.mock('../../../stores/configStore', () => ({
  useConfigStore: (selector: (state: { config: { modelDownloadMirror: string } }) => unknown) =>
    selector({ config: { modelDownloadMirror: 'direct' } }),
}));

vi.mock('../../../stores/dialogStore', () => ({
  useDialogStore: (
    selector: (state: { confirm: (opts: unknown) => Promise<boolean> }) => unknown
  ) => selector({ confirm: vi.fn().mockResolvedValue(true) }),
}));

const mockConfig: LlmAssistantConfig = {
  llmSettings: createLlmSettings(),
};

describe('LocalProviderAccordionItem', () => {
  const defaultT = (key: string, options?: Record<string, unknown>) =>
    (options?.defaultValue as string) || key;

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders import GGUF and refresh buttons with project unified data-tooltip instead of title', () => {
    render(
      <LocalProviderAccordionItem
        config={mockConfig}
        isOpen={true}
        onToggle={vi.fn()}
        applyLlmSettings={vi.fn()}
        t={defaultT}
      />
    );

    const importBtn = screen.getByRole('button', { name: /导入本地 GGUF/i });
    expect(importBtn).toBeTruthy();
    expect(importBtn.getAttribute('data-tooltip')).toBe('导入本地已有的 GGUF 格式模型文件');
    expect(importBtn.getAttribute('data-tooltip-pos')).toBe('top');
    expect(importBtn.getAttribute('title')).toBeNull();

    const refreshBtn = screen.getByRole('button', { name: /刷新/i });
    expect(refreshBtn).toBeTruthy();
    expect(refreshBtn.getAttribute('data-tooltip')).toBe('刷新');
    expect(refreshBtn.getAttribute('data-tooltip-pos')).toBe('top');
    expect(refreshBtn.getAttribute('title')).toBeNull();
  });

  it('calls openDialog and importLocalLlmFile when importing custom GGUF', async () => {
    vi.mocked(openDialog).mockResolvedValue('/downloads/custom-model.gguf');

    render(
      <LocalProviderAccordionItem
        config={mockConfig}
        isOpen={true}
        onToggle={vi.fn()}
        applyLlmSettings={vi.fn()}
        t={defaultT}
      />
    );

    const importBtn = screen.getByRole('button', { name: /导入本地 GGUF/i });
    fireEvent.click(importBtn);

    await vi.waitFor(() => {
      expect(openDialog).toHaveBeenCalled();
      expect(importLocalLlmFile).toHaveBeenCalledWith('/downloads/custom-model.gguf');
      expect(listLocalLlmCards).toHaveBeenCalled();
    });
  });
});
