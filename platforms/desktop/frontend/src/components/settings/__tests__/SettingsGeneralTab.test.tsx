import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { shimService } from '../../../services/shimService';
import { SettingsGeneralTab } from '../SettingsGeneralTab';

vi.mock('../../../services/shimService', () => ({
  shimService: {
    getStatus: vi.fn(),
    install: vi.fn(),
    uninstall: vi.fn(),
    setDefaultChannel: vi.fn(),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      return String(options?.defaultValue ?? key);
    },
    i18n: { language: 'en' },
  }),
}));

const mockShimService = vi.mocked(shimService);

describe('SettingsGeneralTab - Command Line Tools', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders unconfigured state and installs to PATH on click', async () => {
    mockShimService.getStatus.mockResolvedValue({
      shimDir: 'C:\\Users\\test\\.sona\\bin',
      inCurrentPath: false,
      inUserPath: false,
      isConfigured: false,
      installedShims: [],
      defaultChannel: 'stable',
      currentChannel: 'stable',
      stableInstalled: true,
      nightlyInstalled: false,
    });
    mockShimService.install.mockResolvedValue(undefined);

    await act(async () => {
      render(<SettingsGeneralTab isVisible={true} />);
    });

    expect(screen.getByText('settings.cli_tools.status_title')).toBeDefined();
    expect(screen.getByText('settings.cli_tools.status_not_configured')).toBeDefined();
    // Single channel should NOT show coexistence channel selection
    expect(screen.queryByText('settings.cli_tools.default_channel_title')).toBeNull();

    const installBtn = screen.getByText('settings.cli_tools.install_btn');
    await act(async () => {
      fireEvent.click(installBtn);
    });

    expect(mockShimService.install).toHaveBeenCalled();
  });

  it('renders configured state and uninstalls on click', async () => {
    mockShimService.getStatus.mockResolvedValue({
      shimDir: 'C:\\Users\\test\\.sona\\bin',
      inCurrentPath: true,
      inUserPath: true,
      isConfigured: true,
      installedShims: ['sona-cli.exe', 'sona-mcp.exe'],
      defaultChannel: 'stable',
      currentChannel: 'stable',
      stableInstalled: true,
      nightlyInstalled: false,
    });
    mockShimService.uninstall.mockResolvedValue(undefined);

    await act(async () => {
      render(<SettingsGeneralTab isVisible={true} />);
    });

    expect(screen.getByText('settings.cli_tools.status_configured')).toBeDefined();

    const uninstallBtn = screen.getByText('settings.cli_tools.uninstall_btn');
    await act(async () => {
      fireEvent.click(uninstallBtn);
    });

    expect(mockShimService.uninstall).toHaveBeenCalled();
  });

  it('renders dual channel coexistence options and switches default channel', async () => {
    mockShimService.getStatus.mockResolvedValue({
      shimDir: 'C:\\Users\\test\\.sona\\bin',
      inCurrentPath: true,
      inUserPath: true,
      isConfigured: true,
      installedShims: ['sona-cli.exe', 'sona-mcp.exe'],
      defaultChannel: 'stable',
      currentChannel: 'stable',
      stableInstalled: true,
      nightlyInstalled: true,
    });
    mockShimService.setDefaultChannel.mockResolvedValue(undefined);

    await act(async () => {
      render(<SettingsGeneralTab isVisible={true} />);
    });

    expect(screen.getByText('settings.cli_tools.default_channel_title')).toBeDefined();
    const nightlyRadio = screen.getByLabelText('settings.cli_tools.channel_nightly');
    expect(nightlyRadio).toBeDefined();

    await act(async () => {
      fireEvent.click(nightlyRadio);
    });

    expect(mockShimService.setDefaultChannel).toHaveBeenCalledWith('nightly');
  });

  it('disables the action button while initial status is loading', async () => {
    const { promise } = Promise.withResolvers<never>();
    mockShimService.getStatus.mockReturnValue(promise);

    await act(async () => {
      render(<SettingsGeneralTab isVisible={true} />);
    });

    const installBtn = screen.getByText('settings.cli_tools.install_btn');
    expect(installBtn.hasAttribute('disabled')).toBe(true);
  });

  it('displays an error message when installation fails', async () => {
    mockShimService.getStatus.mockResolvedValue({
      shimDir: 'C:\\Users\\test\\.sona\\bin',
      inCurrentPath: false,
      inUserPath: false,
      isConfigured: false,
      installedShims: [],
      defaultChannel: 'stable',
      currentChannel: 'stable',
      stableInstalled: true,
      nightlyInstalled: false,
    });
    mockShimService.install.mockRejectedValue(new Error('Permission denied'));

    await act(async () => {
      render(<SettingsGeneralTab isVisible={true} />);
    });

    const installBtn = screen.getByText('settings.cli_tools.install_btn');
    await act(async () => {
      fireEvent.click(installBtn);
    });

    expect(screen.getByRole('alert')).toBeDefined();
    expect(screen.getByText('Permission denied')).toBeDefined();
  });

  it('does not change uninstall button text to removing when switching channels', async () => {
    mockShimService.getStatus.mockResolvedValue({
      shimDir: 'C:\\Users\\test\\.sona\\bin',
      inCurrentPath: true,
      inUserPath: true,
      isConfigured: true,
      installedShims: ['sona-cli.exe', 'sona-mcp.exe'],
      defaultChannel: 'stable',
      currentChannel: 'stable',
      stableInstalled: true,
      nightlyInstalled: true,
    });
    const { promise } = Promise.withResolvers<void>();
    mockShimService.setDefaultChannel.mockReturnValue(promise);

    await act(async () => {
      render(<SettingsGeneralTab isVisible={true} />);
    });

    const nightlyRadio = screen.getByLabelText('settings.cli_tools.channel_nightly');
    await act(async () => {
      fireEvent.click(nightlyRadio);
    });

    // Button should still say uninstall_btn, NOT uninstalling
    expect(screen.getByText('settings.cli_tools.uninstall_btn')).toBeDefined();
    expect(screen.queryByText('settings.cli_tools.uninstalling')).toBeNull();
  });

  it('renders a retry button when initial getStatus fails, and clears error on successful retry', async () => {
    mockShimService.getStatus.mockRejectedValueOnce(new Error('Backend offline'));

    await act(async () => {
      render(<SettingsGeneralTab isVisible={true} />);
    });

    expect(screen.getByRole('alert')).toBeDefined();
    expect(screen.getByText('Backend offline')).toBeDefined();
    const retryBtn = screen.getByText('common.retry');
    expect(retryBtn).toBeDefined();

    mockShimService.getStatus.mockResolvedValueOnce({
      shimDir: 'C:\\Users\\test\\.sona\\bin',
      inCurrentPath: true,
      inUserPath: true,
      isConfigured: true,
      installedShims: ['sona-cli.exe', 'sona-mcp.exe'],
      defaultChannel: 'stable',
      currentChannel: 'stable',
      stableInstalled: true,
      nightlyInstalled: false,
    });

    await act(async () => {
      fireEvent.click(retryBtn);
    });

    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.getByText('settings.cli_tools.uninstall_btn')).toBeDefined();
  });
});
