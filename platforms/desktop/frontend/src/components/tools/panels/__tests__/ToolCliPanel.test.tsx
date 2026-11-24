import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cliService } from '../../../../services/cliService';
import { useCliStore } from '../../../../stores/cliStore';
import type { CliStatus, ShellCompletionStatus } from '../../../../types/cli';
import { defaultShellForPlatform, ToolCliPanel } from '../ToolCliPanel';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactI18Next>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, opts?: Record<string, unknown>) => {
        const template = typeof opts?.defaultValue === 'string' ? opts.defaultValue : key;
        return template.replace(/\{\{\s*(\w+)\s*\}\}/g, (_match, name: string) =>
          opts?.[name] !== undefined ? String(opts[name]) : ''
        );
      },
      i18n: { language: 'en' },
    }),
  };
});

vi.mock('../../../../services/cliService', () => ({
  cliService: {
    getStatus: vi.fn(),
    setPathEnabled: vi.fn(),
    switchPathToCurrent: vi.fn(),
    removePathEntry: vi.fn(),
    getCompletionScript: vi.fn(),
    getCompletionStatuses: vi.fn(),
    installCompletion: vi.fn(),
    uninstallCompletion: vi.fn(),
  },
}));

describe('ToolCliPanel', () => {
  const mockStatus: CliStatus = {
    installed: true,
    inPath: false,
    pathState: 'not_in_path',
    binaryPath: 'C:\\Program Files\\Sona\\sona-cli.exe',
    binDir: 'C:\\Program Files\\Sona\\bin',
    version: '0.10.5',
    platform: 'windows',
    installType: 'installed_root',
    canModifyPath: true,
    blockedBySystemPath: false,
    activeEntry: null,
    pathEntries: [],
  };

  const mockCompletionStatuses: ShellCompletionStatus[] = [
    {
      shell: 'powershell',
      state: 'not_installed',
      scriptPath: 'C:\\Users\\test\\.config\\sona\\completions\\sona-cli.ps1',
      profilePaths: [
        'C:\\Users\\test\\Documents\\WindowsPowerShell\\Microsoft.PowerShell_profile.ps1',
      ],
      installedVersion: null,
      currentVersion: '0.10.5',
      profileManaged: false,
      hasLegacyInlineBlock: false,
      manualScriptCmd:
        'New-Item -ItemType Directory -Force "$HOME\\.config\\sona\\completions" | Out-Null; sona-cli completion powershell | Out-File -Encoding utf8 "$HOME\\.config\\sona\\completions\\sona-cli.ps1"',
      manualProfileLine:
        'if (Test-Path "$HOME\\.config\\sona\\completions\\sona-cli.ps1") { . "$HOME\\.config\\sona\\completions\\sona-cli.ps1" }',
    },
    {
      shell: 'zsh',
      state: 'not_installed',
      scriptPath: '/home/test/.zfunc/_sona-cli',
      profilePaths: ['/home/test/.zshrc'],
      installedVersion: null,
      currentVersion: '0.10.5',
      profileManaged: false,
      hasLegacyInlineBlock: false,
      manualScriptCmd: 'mkdir -p ~/.zfunc && sona-cli completion zsh > ~/.zfunc/_sona-cli',
      manualProfileLine: 'fpath=(~/.zfunc $fpath); autoload -Uz compinit && compinit',
    },
    {
      shell: 'bash',
      state: 'not_installed',
      scriptPath: '/home/test/.local/share/bash-completion/completions/sona-cli',
      profilePaths: [],
      installedVersion: null,
      currentVersion: '0.10.5',
      profileManaged: true,
      hasLegacyInlineBlock: false,
      manualScriptCmd:
        'mkdir -p ~/.local/share/bash-completion/completions && sona-cli completion bash > ~/.local/share/bash-completion/completions/sona-cli',
      manualProfileLine: null,
    },
    {
      shell: 'fish',
      state: 'not_installed',
      scriptPath: '/home/test/.config/fish/completions/sona-cli.fish',
      profilePaths: [],
      installedVersion: null,
      currentVersion: '0.10.5',
      profileManaged: true,
      hasLegacyInlineBlock: false,
      manualScriptCmd:
        'mkdir -p ~/.config/fish/completions && sona-cli completion fish > ~/.config/fish/completions/sona-cli.fish',
      manualProfileLine: null,
    },
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    useCliStore.setState({
      status: mockStatus,
      completionStatuses: Object.fromEntries(
        mockCompletionStatuses.map((s) => [s.shell, s])
      ) as Record<ShellCompletionStatus['shell'], ShellCompletionStatus>,
      isLoading: false,
      isTogglingPath: false,
      removingPathDir: null,
      busyCompletionShell: null,
      error: null,
      pathJustUpdated: false,
      completionNotice: null,
    });
    vi.mocked(cliService.getStatus).mockResolvedValue(mockStatus);
    vi.mocked(cliService.getCompletionStatuses).mockResolvedValue(mockCompletionStatuses);
    vi.mocked(cliService.setPathEnabled).mockResolvedValue({
      ...mockStatus,
      inPath: true,
      pathState: 'active_current',
    });
  });

  afterEach(() => {
    cleanup();
  });

  it('renders hero header and path integration section', async () => {
    render(<ToolCliPanel />);

    expect(screen.getByText('Sona CLI')).toBeDefined();
    expect(screen.getByText('System PATH Integration')).toBeDefined();
    expect(screen.getByText('C:\\Program Files\\Sona\\bin')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Add to System PATH' })).toBeDefined();
  });

  it('calls setPathEnabled when clicking Add to System PATH', async () => {
    render(<ToolCliPanel />);

    const addButton = screen.getByRole('button', { name: 'Add to System PATH' });
    fireEvent.click(addButton);

    expect(cliService.setPathEnabled).toHaveBeenCalledWith(true);

    await waitFor(() => {
      expect(screen.getByText(/Environment Variable Updated/i)).toBeDefined();
    });
  });

  it('shows multi-environment warning, active entry, and calls switchPathToCurrent', async () => {
    const multiEnvStatus: CliStatus = {
      ...mockStatus,
      inPath: false,
      pathState: 'shadowed_by_other',
      blockedBySystemPath: false,
      activeEntry: {
        pathDir: 'D:\\Portable\\Sona\\bin',
        entryBinaryPath: 'D:\\Portable\\Sona\\bin\\sona-cli.exe',
        resolvedBinaryPath: 'D:\\Portable\\Sona\\sona-cli.exe',
        version: '0.10.2',
        source: 'user_path',
        isActive: true,
        isCurrentApp: false,
        canRemove: true,
        isBroken: false,
      },
      pathEntries: [
        {
          pathDir: 'D:\\Portable\\Sona\\bin',
          entryBinaryPath: 'D:\\Portable\\Sona\\bin\\sona-cli.exe',
          resolvedBinaryPath: 'D:\\Portable\\Sona\\sona-cli.exe',
          version: '0.10.2',
          source: 'user_path',
          isActive: true,
          isCurrentApp: false,
          canRemove: true,
          isBroken: false,
        },
        {
          pathDir: 'C:\\Program Files\\Sona\\bin',
          entryBinaryPath: 'C:\\Program Files\\Sona\\bin\\sona-cli.exe',
          resolvedBinaryPath: 'C:\\Program Files\\Sona\\sona-cli.exe',
          version: '0.10.5',
          source: 'user_path',
          isActive: false,
          isCurrentApp: true,
          canRemove: true,
          isBroken: false,
        },
      ],
    };

    vi.mocked(cliService.getStatus).mockResolvedValue(multiEnvStatus);
    vi.mocked(cliService.switchPathToCurrent).mockResolvedValue({
      ...mockStatus,
      inPath: true,
      pathState: 'active_current',
      activeEntry: multiEnvStatus.pathEntries[1],
      pathEntries: [multiEnvStatus.pathEntries[1]],
    });
    useCliStore.setState({ status: multiEnvStatus });

    render(<ToolCliPanel />);

    expect(screen.getByText('Another Sona Environment Is Active in Terminals')).toBeDefined();
    expect(screen.getAllByText('D:\\Portable\\Sona\\bin').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('Detected Environments in PATH')).toBeDefined();
    expect(screen.getByText('#1')).toBeDefined();
    expect(screen.getByText('#2')).toBeDefined();

    const switchBtn = screen.getByRole('button', { name: 'Switch PATH to Current App' });
    fireEvent.click(switchBtn);

    expect(cliService.switchPathToCurrent).toHaveBeenCalledTimes(1);
    await waitFor(() => {
      expect(screen.getByText(/Environment Variable Updated/i)).toBeDefined();
    });
  });

  it('disables switching when blocked by system PATH and allows removing removable entries', async () => {
    const blockedStatus: CliStatus = {
      ...mockStatus,
      inPath: false,
      pathState: 'shadowed_by_other',
      blockedBySystemPath: true,
      activeEntry: {
        pathDir: 'C:\\SystemSona\\bin',
        entryBinaryPath: 'C:\\SystemSona\\bin\\sona-cli.exe',
        resolvedBinaryPath: 'C:\\SystemSona\\sona-cli.exe',
        version: '0.10.1',
        source: 'system_path',
        isActive: true,
        isCurrentApp: false,
        canRemove: false,
        isBroken: false,
      },
      pathEntries: [
        {
          pathDir: 'C:\\SystemSona\\bin',
          entryBinaryPath: 'C:\\SystemSona\\bin\\sona-cli.exe',
          resolvedBinaryPath: 'C:\\SystemSona\\sona-cli.exe',
          version: '0.10.1',
          source: 'system_path',
          isActive: true,
          isCurrentApp: false,
          canRemove: false,
          isBroken: false,
        },
        {
          pathDir: 'C:\\Program Files\\Sona\\bin',
          entryBinaryPath: 'C:\\Program Files\\Sona\\bin\\sona-cli.exe',
          resolvedBinaryPath: 'C:\\Program Files\\Sona\\sona-cli.exe',
          version: '0.10.5',
          source: 'user_path',
          isActive: false,
          isCurrentApp: true,
          canRemove: true,
          isBroken: false,
        },
      ],
    };

    vi.mocked(cliService.getStatus).mockResolvedValue(blockedStatus);
    vi.mocked(cliService.removePathEntry).mockResolvedValue({
      ...blockedStatus,
      pathEntries: [blockedStatus.pathEntries[0]],
      pathState: 'active_other',
    });
    useCliStore.setState({ status: blockedStatus });

    render(<ToolCliPanel />);

    expect(screen.getByText(/system-level PATH entry takes precedence/i)).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Switch PATH to Current App' })).toBeNull();

    const heroSwitch = screen.getByRole('switch', { name: 'Toggle Sona CLI in PATH' });
    expect(heroSwitch.getAttribute('aria-disabled')).toBe('true');

    // Only the user_path entry renders a Remove button
    const removeBtns = screen.getAllByRole('button', { name: 'Remove' });
    expect(removeBtns).toHaveLength(1);
    fireEvent.click(removeBtns[0]);
    expect(cliService.removePathEntry).toHaveBeenCalledWith('C:\\Program Files\\Sona\\bin');
  });

  it('renders error banner when store contains error and dismisses it', async () => {
    vi.mocked(cliService.getStatus).mockRejectedValueOnce(new Error('Failed to access registry'));
    render(<ToolCliPanel />);

    await waitFor(() => {
      expect(screen.getByText(/Failed to access registry/i)).toBeDefined();
    });
    const dismissBtn = screen.getByRole('button', { name: 'Dismiss' });
    fireEvent.click(dismissBtn);

    expect(useCliStore.getState().error).toBeNull();
  });

  it('resolves default shell by platform correctly', () => {
    expect(defaultShellForPlatform('macos')).toBe('zsh');
    expect(defaultShellForPlatform('linux')).toBe('bash');
    expect(defaultShellForPlatform('windows')).toBe('powershell');
    expect(defaultShellForPlatform(undefined)).toBe('powershell');
  });

  it('defaults shell tab to Zsh on macOS and Bash on Linux', async () => {
    const macStatus = { ...mockStatus, platform: 'macos' as const };
    vi.mocked(cliService.getStatus).mockResolvedValue(macStatus);
    useCliStore.setState({ status: macStatus });
    const { unmount } = render(<ToolCliPanel />);
    await waitFor(() => {
      expect(screen.getAllByText(/~\/\.zfunc\/_sona-cli/).length).toBeGreaterThanOrEqual(1);
    });
    unmount();

    const linuxStatus = { ...mockStatus, platform: 'linux' as const };
    vi.mocked(cliService.getStatus).mockResolvedValue(linuxStatus);
    useCliStore.setState({ status: linuxStatus });
    render(<ToolCliPanel />);
    await waitFor(() => {
      expect(
        screen.getAllByText(/bash-completion\/completions\/sona-cli/).length
      ).toBeGreaterThanOrEqual(1);
    });
  });

  it('switches shell completions tabs and never uses inline Out-File append to $PROFILE', async () => {
    const { container } = render(<ToolCliPanel />);
    await waitFor(() => {
      expect(cliService.getStatus).toHaveBeenCalled();
    });

    const commandText = Array.from(container.querySelectorAll('.tool-cli-command'))
      .map((el) => el.textContent ?? '')
      .join('\n');
    expect(commandText).not.toContain('Out-File -Append');
    expect(commandText).toContain('sona-cli.ps1');
    expect(commandText).toContain('Test-Path');

    const zshTab = screen.getByRole('tab', { name: 'Zsh' });
    fireEvent.click(zshTab);

    expect(screen.getAllByText(/~\/\.zfunc\/_sona-cli/).length).toBeGreaterThanOrEqual(1);

    const fishTab = screen.getByRole('tab', { name: 'Fish' });
    fireEvent.click(fishTab);

    expect(screen.getAllByText(/fish\/completions\/sona-cli\.fish/).length).toBeGreaterThanOrEqual(
      1
    );
  });

  it('supports one-click completion install, uninstall, and legacy inline warning', async () => {
    const legacyPs: ShellCompletionStatus = {
      ...mockCompletionStatuses[0],
      state: 'outdated',
      installedVersion: '0.10.2',
      hasLegacyInlineBlock: true,
    };
    const installedPs: ShellCompletionStatus = {
      ...mockCompletionStatuses[0],
      state: 'installed',
      installedVersion: '0.10.5',
      profileManaged: true,
      hasLegacyInlineBlock: true,
    };

    const statusesWithLegacy = [legacyPs, ...mockCompletionStatuses.slice(1)];
    vi.mocked(cliService.getCompletionStatuses).mockResolvedValue(statusesWithLegacy);
    vi.mocked(cliService.installCompletion).mockResolvedValue(installedPs);
    vi.mocked(cliService.uninstallCompletion).mockResolvedValue(mockCompletionStatuses[0]);

    useCliStore.setState({
      completionStatuses: Object.fromEntries(statusesWithLegacy.map((s) => [s.shell, s])) as Record<
        ShellCompletionStatus['shell'],
        ShellCompletionStatus
      >,
    });

    render(<ToolCliPanel />);

    expect(screen.getByText('Legacy Inline Completion Detected in $PROFILE')).toBeDefined();
    const updateBtn = screen.getByRole('button', { name: 'Update to v0.10.5' });
    fireEvent.click(updateBtn);

    expect(cliService.installCompletion).toHaveBeenCalledWith('powershell');
    await waitFor(() => {
      expect(screen.getByText(/Shell Completion Updated/i)).toBeDefined();
    });

    const uninstallBtn = screen.getByRole('button', { name: 'Uninstall' });
    fireEvent.click(uninstallBtn);
    expect(cliService.uninstallCompletion).toHaveBeenCalledWith('powershell');
  });

  it('renders quick start cheat sheet commands and icon-only copy buttons', () => {
    const { container } = render(<ToolCliPanel />);

    expect(screen.getByText('Batch Transcription')).toBeDefined();
    expect(
      screen.getByText('sona-cli transcribe ./audio.wav -m whisper-turbo -o ./subtitle.srt')
    ).toBeDefined();
    expect(screen.getByText('Live Microphone Recognition')).toBeDefined();

    // Verify cheat sheet copy buttons have borderless icon class and no data-tooltip
    const copyBtns = container.querySelectorAll('.tool-cli-terminal-copy');
    expect(copyBtns.length).toBe(5);
    for (const btn of copyBtns) {
      expect(btn.classList.contains('btn-icon')).toBe(true);
      expect(btn.hasAttribute('data-tooltip')).toBe(false);
    }
  });

  it('renders bin directory path with unified custom tooltip', () => {
    const { container } = render(<ToolCliPanel />);
    const tooltipWrapper = container.querySelector('.tool-cli-path-tooltip-wrapper');
    expect(tooltipWrapper).not.toBeNull();
    expect(tooltipWrapper?.getAttribute('data-tooltip')).toBe('C:\\Program Files\\Sona\\bin');
    expect(tooltipWrapper?.getAttribute('data-tooltip-pos')).toBe('top');
    expect(tooltipWrapper?.hasAttribute('data-tooltip-multiline')).toBe(true);
  });
});
