import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { LocalLlmModelCard as LocalLlmModelCardType } from '../../../bindings';
import { LocalModelCard } from '../llm/LocalModelCard';

const mockCard: LocalLlmModelCardType = {
  id: 'qwen3.5-4b',
  name: 'Qwen3.5 4B',
  model: 'Qwen/Qwen3.5-4B',
  filename: 'Qwen3.5-4B-Q4_K_M.gguf',
  description: 'settings.descriptions.qwen3_5_4b',
  backend: 'llama.cpp',
  contextWindow: 262144,
  maxOutputTokens: 4096,
  size: '~2.7 GB',
  parameters: '4B',
  quantization: 'Q4_K_M',
  languages: ['zh', 'en', 'ja', 'ko'],
  capabilities: ['chat', 'reasoning', 'polish', 'summary', 'translate'],
  isRecommended: true,
  isInstalled: false,
  installedPath: null,
  installedSizeBytes: null,
  downloadUrl: 'https://example.com/model.gguf',
  downloadSizeBytes: 2740937888,
};

describe('LocalModelCard', () => {
  const defaultT = (key: string, options?: Record<string, unknown>) =>
    (options?.defaultValue as string) || key;

  it('renders model card with specs, tags, and download button when not installed', () => {
    const onDownload = vi.fn();
    const onCancelDownload = vi.fn();
    const onDelete = vi.fn();
    const onApplyFeature = vi.fn();

    render(
      <LocalModelCard
        card={mockCard}
        activeFeatures={{ polish: false, translation: false, summary: false }}
        onDownload={onDownload}
        onCancelDownload={onCancelDownload}
        onDelete={onDelete}
        onApplyFeature={onApplyFeature}
        t={defaultT}
      />
    );

    expect(screen.getByText('Qwen3.5 4B')).toBeTruthy();
    expect(screen.getByText('4B')).toBeTruthy();
    expect(screen.getAllByText('Q4_K_M').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('256K (262,144 tokens)')).toBeTruthy();
    expect(screen.getByText('~2.7 GB')).toBeTruthy();

    const downloadBtn = screen.getByRole('button', { name: /点击下载/i });
    expect(downloadBtn).toBeTruthy();
    fireEvent.click(downloadBtn);
    expect(onDownload).toHaveBeenCalledWith(mockCard);
  });

  it('renders progress bar and cancel button when downloading', () => {
    const onCancelDownload = vi.fn();

    render(
      <LocalModelCard
        card={mockCard}
        downloadState={{
          progress: 45,
          statusText: '45% (1.2 GB / 2.7 GB)',
          downloadedBytes: 1288490188,
          totalBytes: 2740937888,
        }}
        activeFeatures={{ polish: false, translation: false, summary: false }}
        onDownload={vi.fn()}
        onCancelDownload={onCancelDownload}
        onDelete={vi.fn()}
        onApplyFeature={vi.fn()}
        t={defaultT}
      />
    );

    expect(screen.getAllByText(/45%/).length).toBeGreaterThanOrEqual(1);
    const cancelBtn = screen.getByRole('button', { name: /取消下载/i });
    expect(cancelBtn).toBeTruthy();
    fireEvent.click(cancelBtn);
    expect(onCancelDownload).toHaveBeenCalledWith('qwen3.5-4b');
  });

  it('renders quick apply buttons and delete button when installed', () => {
    const onApplyFeature = vi.fn();
    const onDelete = vi.fn();
    const installedCard: LocalLlmModelCardType = {
      ...mockCard,
      isInstalled: true,
      installedPath: '/models/Qwen3.5-4B-Q4_K_M.gguf',
      installedSizeBytes: 2740937888,
    };

    render(
      <LocalModelCard
        card={installedCard}
        activeFeatures={{ polish: true, translation: false, summary: false }}
        onDownload={vi.fn()}
        onCancelDownload={vi.fn()}
        onDelete={onDelete}
        onApplyFeature={onApplyFeature}
        t={defaultT}
      />
    );

    expect(screen.getByText('已就绪')).toBeTruthy();
    expect(screen.getByText('/models/Qwen3.5-4B-Q4_K_M.gguf')).toBeTruthy();

    const applyAllBtn = screen.getByRole('button', { name: /设为全部功能模型/i });
    expect(applyAllBtn).toBeTruthy();
    fireEvent.click(applyAllBtn);
    expect(onApplyFeature).toHaveBeenCalledWith(installedCard, 'all');

    const polishBtn = screen.getByRole('button', { name: /润色/i });
    expect(polishBtn).toBeTruthy();
    fireEvent.click(polishBtn);
    expect(onApplyFeature).toHaveBeenCalledWith(installedCard, 'polish');

    const deleteBtn = screen.getByRole('button', { name: /删除模型/i });
    expect(deleteBtn).toBeTruthy();
    fireEvent.click(deleteBtn);
    expect(onDelete).toHaveBeenCalledWith(installedCard);
  });
});
