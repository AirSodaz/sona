import { expect, vi, beforeEach, describe, it } from 'vitest';
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react';
import { SettingsApiServerTab } from '../SettingsApiServerTab';
import { buildTestConfig } from '../../../test-utils/configTestUtils';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      return (options?.defaultValue as string) || key;
    },
  }),
}));

vi.mock('../../tauri/invoke', () => ({
  invokeTauri: vi.fn().mockResolvedValue('OK'),
}));

const mockUpdateConfig = vi.fn();
let currentConfig = buildTestConfig({
  httpServerEnabled: false,
  httpServerHost: '127.0.0.1',
  httpServerPort: 14200,
  httpServerApiKey: 'original-key',
});

vi.mock('../../../stores/configStore', async () => {
  const actual = await vi.importActual<typeof import('../../../stores/configStore')>('../../../stores/configStore');
  return {
    ...actual,
    useApiServerConfig: () => currentConfig,
    useSetConfig: () => mockUpdateConfig,
  };
});

describe('SettingsApiServerTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    currentConfig = buildTestConfig({
      httpServerEnabled: false,
      httpServerHost: '127.0.0.1',
      httpServerPort: 14200,
      httpServerApiKey: 'original-key',
    });
  });

  it('renders all settings elements and sub-labels (hints)', async () => {
    await act(async () => {
      render(<SettingsApiServerTab />);
    });

    expect(screen.getByText('Enable API Server')).toBeDefined();
    expect(screen.getByText('Host')).toBeDefined();
    expect(screen.getByText('Port')).toBeDefined();
    expect(screen.getByText('API Key')).toBeDefined();

    // Verify sub-label hints render
    expect(screen.getByText('Start an HTTP server to control Sona via external applications.')).toBeDefined();
    expect(screen.getByText(/Bind address for the server/)).toBeDefined();
    expect(screen.getByText('TCP port for the API server. Must be between 1 and 65535 (default: 14200).')).toBeDefined();
    expect(screen.getByText('Optional Bearer token for authenticating HTTP requests.')).toBeDefined();
  });

  it('correctly maps the active inputs to the configuration store', async () => {
    await act(async () => {
      render(<SettingsApiServerTab />);
    });

    const hostInput = screen.getByDisplayValue('127.0.0.1') as HTMLInputElement;
    const portInput = screen.getByDisplayValue('14200') as HTMLInputElement;
    const apiKeyInput = screen.getByDisplayValue('original-key') as HTMLInputElement;

    expect(hostInput).toBeDefined();
    expect(portInput).toBeDefined();
    expect(apiKeyInput).toBeDefined();
  });

  it('triggers config updates on host/port/key input changes', async () => {
    await act(async () => {
      render(<SettingsApiServerTab />);
    });

    const hostInput = screen.getByDisplayValue('127.0.0.1');
    await act(async () => {
      fireEvent.change(hostInput, { target: { value: '192.168.1.1' } });
    });
    expect(mockUpdateConfig).toHaveBeenCalledWith({ httpServerHost: '192.168.1.1' });

    const portInput = screen.getByDisplayValue('14200');
    await act(async () => {
      fireEvent.change(portInput, { target: { value: '15000' } });
    });
    expect(mockUpdateConfig).toHaveBeenCalledWith({ httpServerPort: 15000 });

    const apiKeyInput = screen.getByDisplayValue('original-key');
    await act(async () => {
      fireEvent.change(apiKeyInput, { target: { value: 'new-custom-key' } });
    });
    expect(mockUpdateConfig).toHaveBeenCalledWith({ httpServerApiKey: 'new-custom-key' });
  });

  it('handles Generate API Key', async () => {
    await act(async () => {
      render(<SettingsApiServerTab />);
    });

    const generateBtn = screen.getByRole('button', { name: 'Generate' });
    await act(async () => {
      fireEvent.click(generateBtn);
    });

    expect(mockUpdateConfig).toHaveBeenCalled();
    const arg = mockUpdateConfig.mock.calls[0]?.[0];
    expect(arg.httpServerApiKey).toBeDefined();
    expect(arg.httpServerApiKey).not.toBe('original-key');
  });

  it('renders custom data-tooltips on action buttons', async () => {
    await act(async () => {
      render(<SettingsApiServerTab />);
    });

    const generateBtn = screen.getByRole('button', { name: 'Generate' });
    const copyBtn = screen.getByRole('button', { name: 'Copy' });

    expect(generateBtn.getAttribute('data-tooltip')).toBe('Generate');
    expect(generateBtn.getAttribute('data-tooltip-pos')).toBe('top');

    expect(copyBtn.getAttribute('data-tooltip')).toBe('Copy');
    expect(copyBtn.getAttribute('data-tooltip-pos')).toBe('top');
  });

  it('updates dynamic Copy Key tooltip upon clicking copy', async () => {
    // Mock navigator.clipboard
    const mockWriteText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: {
        writeText: mockWriteText,
      },
    });

    await act(async () => {
      render(<SettingsApiServerTab />);
    });

    const copyBtn = screen.getByRole('button', { name: 'Copy' });
    await act(async () => {
      fireEvent.click(copyBtn);
    });

    expect(mockWriteText).toHaveBeenCalledWith('original-key');

    // Tooltip should switch to 'Copied!' reactively
    await waitFor(() => {
      expect(copyBtn.getAttribute('data-tooltip')).toBe('Copied!');
    });
  });
});
