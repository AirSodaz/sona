import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const updateConfig = vi.fn();
const listLlmModels = vi.fn();

let mockLlmConfig: Record<string, unknown> = {
  llmSettings: {
    activeProvider: 'open_ai',
    providers: {
      open_ai: { apiHost: 'https://api.openai.com/v1', apiKey: '' },
      ollama: { apiHost: 'http://localhost:11434', apiKey: '' },
      lm_studio: { apiHost: 'http://localhost:1234/v1', apiKey: '' },
    },
    models: {},
    modelOrder: [],
    selections: {},
    customProviders: {},
  },
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
  }),
  initReactI18next: { type: '3rdParty', init: vi.fn() },
}));

vi.mock('../../../../stores/configStore', () => ({
  useLlmAssistantConfig: () => mockLlmConfig,
  useSetConfig: () => updateConfig,
}));

vi.mock('../../../../services/tauri/llm', () => ({
  listLlmModels: (...args: unknown[]) => listLlmModels(...args),
}));

vi.mock('../../../Modal', () => ({
  Modal: ({
    isOpen,
    children,
    footer,
    title,
  }: {
    isOpen: boolean;
    children: React.ReactNode;
    footer?: React.ReactNode;
    title?: string;
  }) =>
    isOpen ? (
      <div data-testid="connect-modal">
        <h2>{title}</h2>
        {children}
        <div>{footer}</div>
      </div>
    ) : null,
}));

vi.mock('../../../Dropdown', () => ({
  Dropdown: ({
    value,
    onChange,
    options,
    placeholder,
  }: {
    value: string;
    onChange: (value: string) => void;
    options: Array<{ value: string; label: string }>;
    placeholder?: string;
  }) => (
    <select
      aria-label={placeholder || 'model'}
      value={value}
      onChange={(event) => onChange(event.target.value)}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

import { AiWritingConnectModal } from '../AiWritingConnectModal';
import { clearProviderProbeCache } from '../capabilityStatus';

describe('AiWritingConnectModal OpenAI-compatible mode', () => {
  beforeEach(() => {
    updateConfig.mockReset();
    listLlmModels.mockReset();
    clearProviderProbeCache();
    mockLlmConfig = {
      llmSettings: {
        activeProvider: 'open_ai',
        providers: {
          open_ai: { apiHost: 'https://api.openai.com/v1', apiKey: '' },
          ollama: { apiHost: 'http://localhost:11434', apiKey: '' },
          lm_studio: { apiHost: 'http://localhost:1234/v1', apiKey: '' },
        },
        models: {},
        modelOrder: [],
        selections: {},
        customProviders: {},
      },
    };
  });

  it('exposes Compatible mode with optional API key and probes via custom provider', async () => {
    listLlmModels.mockResolvedValue([{ model: 'proxy-mini' }]);

    render(<AiWritingConnectModal isOpen onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole('radio', { name: 'Compatible' }));
    expect(screen.getByText(/OpenAI-compatible chat completions/i)).toBeTruthy();
    expect(screen.getByText('API Key (optional)')).toBeTruthy();

    const host = screen.getByPlaceholderText('https://example.com/v1');
    fireEvent.change(host, { target: { value: 'http://127.0.0.1:8080/v1' } });

    fireEvent.click(screen.getByRole('button', { name: 'Test connection' }));

    await waitFor(() => {
      expect(listLlmModels).toHaveBeenCalled();
    });

    const probeArgs = listLlmModels.mock.calls[0][0] as {
      provider: string;
      strategy: string;
      baseUrl: string;
      apiKey: string;
    };
    expect(probeArgs.strategy).toBe('openai_compatible');
    expect(probeArgs.baseUrl).toBe('http://127.0.0.1:8080/v1');
    expect(probeArgs.apiKey).toBe('');
    expect(String(probeArgs.provider)).toMatch(/^custom-/);

    await waitFor(() => {
      expect(screen.getByDisplayValue('proxy-mini')).toBeTruthy();
    });
  });

  it('keeps OpenAI / Ollama / LM Studio presets', () => {
    render(<AiWritingConnectModal isOpen onClose={vi.fn()} />);
    expect(screen.getByRole('radio', { name: 'OpenAI' })).toBeTruthy();
    expect(screen.getByRole('radio', { name: 'Ollama' })).toBeTruthy();
    expect(screen.getByRole('radio', { name: 'LM Studio' })).toBeTruthy();
    expect(screen.getByRole('radio', { name: 'Compatible' })).toBeTruthy();
  });
});
