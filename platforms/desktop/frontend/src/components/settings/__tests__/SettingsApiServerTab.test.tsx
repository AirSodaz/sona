import { expect, vi, beforeEach, describe, it } from 'vitest';
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react';
import { SettingsApiServerTab } from '../SettingsApiServerTab';
import { buildTestConfig } from '../../../test-utils/configTestUtils';
import { invokeTauri } from '../../../services/tauri/invoke';
import { TauriCommand } from '../../../services/tauri/commands';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, options?: Record<string, unknown>) => {
        return (options?.defaultValue as string) || key;
      },
    }),
  };
});

vi.mock('../../../services/tauri/invoke', () => ({
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

  it('renders Server Status and Job Queue when enabled', async () => {
    currentConfig.httpServerEnabled = true;

    // Mock fetch
    const mockFetch = vi.fn().mockImplementation((url: string) => {
        if (url.endsWith('/health')) {
            return Promise.resolve({
                ok: true,
                json: () => Promise.resolve({ status: 'ok', version: '1.0.0', uptime: 3600 })
            });
        }
        if (url.endsWith('/info')) {
            return Promise.resolve({
                ok: true,
                json: () => Promise.resolve({ platform: 'win32', gpuAvailable: true, models: [], vadInstalled: true, punctuationInstalled: true })
            });
        }
        if (url.endsWith('/jobs')) {
            return Promise.resolve({
                ok: true,
                json: () => Promise.resolve({ 'test-job': 'Processing' })
            });
        }
        return Promise.reject(new Error('Unknown URL'));
    });
    global.fetch = mockFetch as any;

    await act(async () => {
      render(<SettingsApiServerTab />);
    });

    expect(screen.getByText('Server Status')).toBeDefined();
    expect(screen.getByText('Job Queue')).toBeDefined();

    await waitFor(() => {
        expect(screen.getByText('Running')).toBeDefined();
        expect(screen.getByText('1h 0m 0s')).toBeDefined();
        expect(screen.getByText('Processing')).toBeDefined();
    });
  });

  it('starts the API server with GPU acceleration, the 50MB upload default, and normalized whitelist sync', async () => {
    vi.useFakeTimers();
    vi.mocked(invokeTauri).mockResolvedValueOnce('127.0.0.0/8,::1/128');
    currentConfig = buildTestConfig({
      httpServerEnabled: true,
      httpServerHost: '127.0.0.1',
      httpServerPort: 14200,
      httpServerApiKey: 'original-key',
      gpuAcceleration: 'cuda',
      httpServerMaxUploadSizeMB: undefined,
      httpServerIpWhitelist: 'localhost',
    });
    global.fetch = vi.fn().mockResolvedValue({ ok: false }) as any;

    await act(async () => {
      render(<SettingsApiServerTab />);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(invokeTauri).toHaveBeenCalledWith(TauriCommand.apiServer.start, expect.objectContaining({
      gpuAcceleration: 'cuda',
      maxUploadSizeMb: 50,
    }));
    expect(mockUpdateConfig).toHaveBeenCalledWith({ httpServerIpWhitelist: '127.0.0.0/8,::1/128' });
    vi.useRealTimers();
  });
});
