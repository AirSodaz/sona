import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cliService } from '../../../../services/cliService';
import { useCliStore } from '../../../../stores/cliStore';
import { ToolCliPanel } from '../ToolCliPanel';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactI18Next>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue ?? key,
      i18n: { language: 'en' },
    }),
  };
});

vi.mock('../../../../services/cliService', () => ({
  cliService: {
    getStatus: vi.fn(),
    setPathEnabled: vi.fn(),
    getCompletionScript: vi.fn(),
  },
}));

describe('ToolCliPanel', () => {
  const mockStatus = {
    installed: true,
    inPath: false,
    binaryPath: 'C:\\Program Files\\Sona\\sona-cli.exe',
    binDir: 'C:\\Program Files\\Sona\\bin',
    version: '0.10.3',
    platform: 'windows' as const,
    installType: 'installed_with_bin',
    canModifyPath: true,
  };

  beforeEach(() => {
    vi.clearAllMocks();
    useCliStore.setState({
      status: mockStatus,
      isLoading: false,
      isTogglingPath: false,
      error: null,
      pathJustUpdated: false,
    });
    vi.mocked(cliService.getStatus).mockResolvedValue(mockStatus);
    vi.mocked(cliService.setPathEnabled).mockResolvedValue({
      ...mockStatus,
      inPath: true,
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

  it('switches shell completions tabs', () => {
    render(<ToolCliPanel />);

    const zshTab = screen.getByRole('tab', { name: 'Zsh' });
    fireEvent.click(zshTab);

    expect(screen.getByText(/~\/\.zfunc\/_sona-cli/)).toBeDefined();

    const fishTab = screen.getByRole('tab', { name: 'Fish' });
    fireEvent.click(fishTab);

    expect(screen.getByText(/fish\/completions\/sona-cli\.fish/)).toBeDefined();
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
