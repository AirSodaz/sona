import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createLlmSettings } from '../../../services/llm/state';
import { completeLlm, listLocalLlmCards, listLocalNmtCards } from '../../../services/tauri/llm';
import { useModelDownloadStore } from '../../../stores/modelDownloadStore';
import type { LlmAssistantConfig } from '../../../types/config';
import type { LlmCompletionResponse } from '../../../types/llm';
import { LocalProviderAccordionItem } from '../llm/LocalProviderAccordionItem';

vi.mock('../../../services/tauri/llm', () => ({
  listLocalLlmCards: vi.fn().mockResolvedValue({ modelsDir: '/test/models', cards: [] }),
  listLocalNmtCards: vi.fn().mockResolvedValue({ modelsDir: '/test/models', cards: [] }),
  testLocalNmtModel: vi.fn().mockResolvedValue('test translation'),
  completeLlm: vi.fn().mockImplementation(async () => ({
    text: 'test response',
    usage: {
      completionTokens: 30,
      promptTokens: 10,
      totalTokens: 40,
      cachedInputTokens: 0,
      cacheCreationInputTokens: 0,
      reasoningTokens: 0,
    },
    execution: {
      requestedFormat: 'text',
      appliedFormat: 'text',
      warnings: [],
      attempts: 1,
    },
  })),
  importLocalLlmFile: vi.fn().mockResolvedValue('/test/models/custom-model.gguf'),
}));

vi.mock('../../../services/tauri/app', () => ({
  downloadPresetModel: vi.fn(),
  cancelDownload: vi.fn(),
  deletePresetModel: vi.fn(),
  cleanInterruptedDownload: vi.fn().mockResolvedValue(true),
}));

vi.mock('../../../services/tauri/platform/dialog', () => ({
  openDialog: vi.fn(),
}));

vi.mock('../../../stores/configStore', () => ({
  useConfigStore: (selector: (state: { config: { modelDownloadMirror: string } }) => unknown) =>
    selector({ config: { modelDownloadMirror: 'direct' } }),
}));

const mockConfirm = vi.fn().mockResolvedValue(true);
const mockCheckboxConfirm = vi.fn().mockResolvedValue({ confirmed: true, checked: false });
const mockShowError = vi.fn().mockResolvedValue(undefined);

vi.mock('../../../stores/dialogStore', () => ({
  useDialogStore: (
    selector: (state: {
      confirm: typeof mockConfirm;
      checkboxConfirm: typeof mockCheckboxConfirm;
      alert: (opts: unknown) => Promise<void>;
      showError: typeof mockShowError;
    }) => unknown
  ) =>
    selector({
      confirm: mockConfirm,
      checkboxConfirm: mockCheckboxConfirm,
      alert: vi.fn().mockResolvedValue(undefined),
      showError: mockShowError,
    }),
}));

const mockConfig: LlmAssistantConfig = {
  llmSettings: createLlmSettings(),
};

const createMockCompletionResponse = (completionTokens = 30): LlmCompletionResponse => ({
  text: '你好！我是 Qwen 模型。',
  usage: {
    promptTokens: 10,
    completionTokens,
    totalTokens: 10 + completionTokens,
    cachedInputTokens: 0,
    cacheCreationInputTokens: 0,
    reasoningTokens: 0,
  },
  execution: {
    requestedFormat: 'text',
    appliedFormat: 'text',
    warnings: [],
    attempts: 1,
  },
});

const mockInstalledCard = {
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
  modalities: ['text', 'image', 'video'],
  languages: ['zh', 'en', 'ja', 'ko'],
  capabilities: ['chat', 'reasoning', 'polish', 'summary', 'translate'],
  isRecommended: true,
  isInstalled: true,
  installedPath: '/test/models/Qwen3.5-4B-Q4_K_M.gguf',
  installedSizeBytes: 2740937888,
  downloadUrl: null,
  downloadSizeBytes: null,
};

describe('LocalProviderAccordionItem', () => {
  const defaultT = (key: string, options?: Record<string, unknown>) =>
    (options?.defaultValue as string) || key;

  beforeEach(() => {
    vi.clearAllMocks();
    useModelDownloadStore.setState({ downloads: {} });
  });

  it('renders LLM and NMT subtabs and toggles between them', async () => {
    const mockNmtCard = {
      ...mockInstalledCard,
      id: 'opus-mt-en-zh',
      name: 'Opus-MT (EN -> ZH)',
      backend: 'onnx-nmt',
      capabilities: ['translate'],
    };

    vi.mocked(listLocalNmtCards).mockResolvedValue({
      modelsDir: '/test/models',
      cards: [mockNmtCard],
    });

    render(
      <LocalProviderAccordionItem
        config={mockConfig}
        isOpen={true}
        onToggle={vi.fn()}
        applyLlmSettings={vi.fn()}
        t={defaultT}
      />
    );

    const llmTab = screen.getByRole('tab', { name: /大语言模型 \(LLM\)/i });
    const nmtTab = screen.getByRole('tab', { name: /翻译模型 \(NMT\)/i });
    expect(llmTab).toBeTruthy();
    expect(nmtTab).toBeTruthy();

    fireEvent.click(nmtTab);
    await vi.waitFor(() => {
      expect(screen.getByText('Opus-MT (EN -> ZH)')).toBeTruthy();
    });
  });

  it('does not render legacy global inference test section even when models are installed', async () => {
    vi.mocked(listLocalLlmCards).mockResolvedValueOnce({
      modelsDir: '/test/models',
      cards: [mockInstalledCard],
    });

    render(
      <LocalProviderAccordionItem
        config={mockConfig}
        isOpen={true}
        onToggle={vi.fn()}
        applyLlmSettings={vi.fn()}
        t={defaultT}
      />
    );

    await vi.waitFor(() => {
      expect(screen.getByText('Qwen3.5 4B')).toBeTruthy();
    });

    expect(screen.queryByText('settings.llm.test_connection_title')).toBeNull();
    expect(screen.queryByText('推理测试')).toBeNull();
    expect(screen.queryByText('运行推理测试')).toBeNull();
  });

  it('triggers per-model connection test with completeLlm, displays speed badge and does not output model text', async () => {
    vi.mocked(listLocalLlmCards).mockResolvedValueOnce({
      modelsDir: '/test/models',
      cards: [mockInstalledCard],
    });

    let currentTime = 1000;
    const perfNowSpy = vi.spyOn(performance, 'now').mockImplementation(() => currentTime);

    vi.mocked(completeLlm).mockImplementationOnce(async () => {
      currentTime = 2200; // 1.2s duration: 30 tokens / 1.2s = 25 t/s
      return createMockCompletionResponse(30);
    });
    render(
      <LocalProviderAccordionItem
        config={mockConfig}
        isOpen={true}
        onToggle={vi.fn()}
        applyLlmSettings={vi.fn()}
        t={defaultT}
      />
    );

    await vi.waitFor(() => {
      const btn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i }) as HTMLButtonElement;
      expect(btn.disabled).toBe(false);
    });

    const testBtn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i });
    await act(async () => {
      fireEvent.click(testBtn);
    });

    await vi.waitFor(() => {
      expect(completeLlm).toHaveBeenCalledWith({
        config: {
          provider: 'local',
          strategy: 'local',
          model: 'Qwen/Qwen3.5-4B',
          baseUrl: '',
          apiKey: '',
          temperature: 0.7,
        },
        input: '你好，请用一句话做自我介绍。',
        source: 'connection_test',
      });
      const speedBadge = screen.getByTestId('local-model-speed-qwen3.5-4b');
      expect(speedBadge.textContent).toBe('25 t/s');
    });

    // Verify model response text is NOT rendered in the UI
    expect(screen.queryByText('你好！我是 Qwen 模型。')).toBeNull();

    const updatedBtn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i });
    expect(updatedBtn.getAttribute('data-tooltip')).toBe('✓ 推理成功 (25 t/s)');

    perfNowSpy.mockRestore();
  });

  it('handles model test failure by showing error state without speed badge or model text', async () => {
    vi.mocked(listLocalLlmCards).mockResolvedValueOnce({
      modelsDir: '/test/models',
      cards: [mockInstalledCard],
    });

    vi.mocked(completeLlm).mockRejectedValueOnce(new Error('GPU out of memory'));

    render(
      <LocalProviderAccordionItem
        config={mockConfig}
        isOpen={true}
        onToggle={vi.fn()}
        applyLlmSettings={vi.fn()}
        t={defaultT}
      />
    );

    await vi.waitFor(() => {
      const btn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i }) as HTMLButtonElement;
      expect(btn.disabled).toBe(false);
    });

    const testBtn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i });
    await act(async () => {
      fireEvent.click(testBtn);
    });

    await vi.waitFor(() => {
      const updatedBtn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i });
      expect(updatedBtn.classList.contains('error')).toBe(true);
      expect(updatedBtn.getAttribute('data-tooltip')).toBe('✕ GPU out of memory');
    });

    expect(screen.queryByTestId('local-model-speed-qwen3.5-4b')).toBeNull();
    expect(screen.queryByText('GPU out of memory')).toBeNull();
  });

  it('resets button status to idle after 3500ms while retaining speed badge', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.mocked(listLocalLlmCards).mockResolvedValueOnce({
      modelsDir: '/test/models',
      cards: [mockInstalledCard],
    });

    vi.mocked(completeLlm).mockResolvedValueOnce(createMockCompletionResponse(30));

    render(
      <LocalProviderAccordionItem
        config={mockConfig}
        isOpen={true}
        onToggle={vi.fn()}
        applyLlmSettings={vi.fn()}
        t={defaultT}
      />
    );

    await vi.waitFor(() => {
      const btn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i }) as HTMLButtonElement;
      expect(btn.disabled).toBe(false);
    });

    const testBtn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i });
    await act(async () => {
      fireEvent.click(testBtn);
    });

    await vi.waitFor(() => {
      const btn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i });
      expect(btn.classList.contains('success')).toBe(true);
    });

    // Advance timer past 3500ms
    act(() => {
      vi.advanceTimersByTime(3500);
    });

    await vi.waitFor(() => {
      const btn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i });
      expect(btn.classList.contains('success')).toBe(false);
      expect(screen.getByTestId('local-model-speed-qwen3.5-4b')).toBeTruthy();
    });

    vi.useRealTimers();
  });

  it('cleans up test state when model is deleted', async () => {
    vi.mocked(listLocalLlmCards)
      .mockResolvedValueOnce({
        modelsDir: '/test/models',
        cards: [mockInstalledCard],
      })
      .mockResolvedValueOnce({
        modelsDir: '/test/models',
        cards: [],
      });

    vi.mocked(completeLlm).mockResolvedValueOnce(createMockCompletionResponse(30));

    render(
      <LocalProviderAccordionItem
        config={mockConfig}
        isOpen={true}
        onToggle={vi.fn()}
        applyLlmSettings={vi.fn()}
        t={defaultT}
      />
    );

    await vi.waitFor(() => {
      const btn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i }) as HTMLButtonElement;
      expect(btn.disabled).toBe(false);
    });

    const testBtn = screen.getByRole('button', { name: /测试连接 Qwen3.5 4B/i });
    await act(async () => {
      fireEvent.click(testBtn);
    });

    await vi.waitFor(() => {
      expect(screen.getByTestId('local-model-speed-qwen3.5-4b')).toBeTruthy();
    });

    // Delete the model
    const deleteBtn = screen.getByRole('button', { name: /删除 Qwen3.5 4B/i });
    await act(async () => {
      fireEvent.click(deleteBtn);
    });

    await vi.waitFor(() => {
      expect(screen.queryByTestId('local-model-speed-qwen3.5-4b')).toBeNull();
    });
  });

  it('resumes download directly without prompt when downloading an interrupted model', async () => {
    const { cleanInterruptedDownload, downloadPresetModel } = await import(
      '../../../services/tauri/app'
    );

    const interruptedCard = {
      ...mockInstalledCard,
      isInstalled: false,
      installedPath: null,
      installedSizeBytes: null,
      interruptedDownload: {
        modelId: 'qwen3.5-4b',
        downloadedBytes: 1370468944,
        totalBytes: 2740937888,
        percentage: 50,
      },
    };

    vi.mocked(listLocalLlmCards).mockResolvedValueOnce({
      modelsDir: '/test/models',
      cards: [interruptedCard],
    });

    render(
      <LocalProviderAccordionItem
        config={mockConfig}
        isOpen={true}
        onToggle={vi.fn()}
        applyLlmSettings={vi.fn()}
        t={defaultT}
      />
    );

    await vi.waitFor(() => {
      expect(screen.getByText(/Paused \(50%\)/)).toBeTruthy();
    });

    const downloadBtn = screen.getByRole('button', { name: /Resume download Qwen3.5 4B/i });
    await act(async () => {
      fireEvent.click(downloadBtn);
    });

    expect(mockCheckboxConfirm).not.toHaveBeenCalled();
    expect(cleanInterruptedDownload).not.toHaveBeenCalled();
    expect(downloadPresetModel).toHaveBeenCalledWith(
      expect.objectContaining({ modelId: 'qwen3.5-4b' })
    );
  });

  it('cleans interrupted download when discard button is clicked and confirmed', async () => {
    const { cleanInterruptedDownload, downloadPresetModel } = await import(
      '../../../services/tauri/app'
    );
    mockConfirm.mockResolvedValueOnce(true);

    const interruptedCard = {
      ...mockInstalledCard,
      isInstalled: false,
      installedPath: null,
      installedSizeBytes: null,
      interruptedDownload: {
        modelId: 'qwen3.5-4b',
        downloadedBytes: 1370468944,
        totalBytes: 2740937888,
        percentage: 50,
      },
    };

    vi.mocked(listLocalLlmCards).mockResolvedValueOnce({
      modelsDir: '/test/models',
      cards: [interruptedCard],
    });

    render(
      <LocalProviderAccordionItem
        config={mockConfig}
        isOpen={true}
        onToggle={vi.fn()}
        applyLlmSettings={vi.fn()}
        t={defaultT}
      />
    );

    await vi.waitFor(() => {
      expect(screen.getByText(/Paused \(50%\)/)).toBeTruthy();
    });

    const discardBtn = screen.getByRole('button', {
      name: /Discard incomplete download Qwen3.5 4B/i,
    });
    await act(async () => {
      fireEvent.click(discardBtn);
    });

    expect(mockConfirm).toHaveBeenCalled();
    expect(cleanInterruptedDownload).toHaveBeenCalledWith('qwen3.5-4b');
    expect(downloadPresetModel).not.toHaveBeenCalled();
  });

  it('does not clean interrupted download when discard button is clicked and cancelled', async () => {
    const { cleanInterruptedDownload, downloadPresetModel } = await import(
      '../../../services/tauri/app'
    );
    mockConfirm.mockResolvedValueOnce(false);

    const interruptedCard = {
      ...mockInstalledCard,
      isInstalled: false,
      installedPath: null,
      installedSizeBytes: null,
      interruptedDownload: {
        modelId: 'qwen3.5-4b',
        downloadedBytes: 1370468944,
        totalBytes: 2740937888,
        percentage: 50,
      },
    };

    vi.mocked(listLocalLlmCards).mockResolvedValueOnce({
      modelsDir: '/test/models',
      cards: [interruptedCard],
    });

    render(
      <LocalProviderAccordionItem
        config={mockConfig}
        isOpen={true}
        onToggle={vi.fn()}
        applyLlmSettings={vi.fn()}
        t={defaultT}
      />
    );

    await vi.waitFor(() => {
      expect(screen.getByText(/Paused \(50%\)/)).toBeTruthy();
    });

    const discardBtn = screen.getByRole('button', {
      name: /Discard incomplete download Qwen3.5 4B/i,
    });
    await act(async () => {
      fireEvent.click(discardBtn);
    });

    expect(mockConfirm).toHaveBeenCalled();
    expect(cleanInterruptedDownload).not.toHaveBeenCalled();
    expect(downloadPresetModel).not.toHaveBeenCalled();
  });
});
