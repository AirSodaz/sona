import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FfmpegInstallGuide } from '../FfmpegInstallGuide';

const mockOpenUrl = vi.fn();

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: (...args: unknown[]) => mockOpenUrl(...args),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue || key,
  }),
}));

vi.mock('lucide-react', () => ({
  HelpCircle: () => null,
  ChevronDown: () => null,
  ChevronRight: () => null,
  Copy: () => null,
  Check: () => null,
  RefreshCw: () => null,
  ExternalLink: () => null,
}));

describe('FfmpegInstallGuide', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.assign(navigator, {
      clipboard: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders Windows instructions when platform is windows', () => {
    render(<FfmpegInstallGuide isValid={false} onRecheck={vi.fn()} platform="windows" />);

    expect(screen.getByText('Windows')).toBeDefined();
    expect(screen.getByText('winget install Gyan.FFmpeg')).toBeDefined();
    expect(screen.getByText('scoop install ffmpeg')).toBeDefined();
    expect(screen.queryByText('brew install ffmpeg')).toBeNull();
    expect(screen.queryByText('sudo apt update && sudo apt install -y ffmpeg')).toBeNull();
  });

  it('renders macOS instructions when platform is macos', () => {
    render(<FfmpegInstallGuide isValid={false} onRecheck={vi.fn()} platform="macos" />);

    expect(screen.getByText('macOS')).toBeDefined();
    expect(screen.getByText('brew install ffmpeg')).toBeDefined();
    expect(screen.queryByText('winget install Gyan.FFmpeg')).toBeNull();
    expect(screen.queryByText('sudo apt update && sudo apt install -y ffmpeg')).toBeNull();
  });

  it('renders Linux instructions when platform is linux', () => {
    render(<FfmpegInstallGuide isValid={false} onRecheck={vi.fn()} platform="linux" />);

    expect(screen.getByText('Linux')).toBeDefined();
    expect(screen.getByText('sudo apt update && sudo apt install -y ffmpeg')).toBeDefined();
    expect(screen.getByText('sudo dnf install -y ffmpeg')).toBeDefined();
    expect(screen.getByText('sudo pacman -S ffmpeg')).toBeDefined();
    expect(screen.queryByText('winget install Gyan.FFmpeg')).toBeNull();
    expect(screen.queryByText('brew install ffmpeg')).toBeNull();
  });

  it('copies command to clipboard when clicking icon-only copy button without tooltip', async () => {
    render(<FfmpegInstallGuide isValid={false} onRecheck={vi.fn()} platform="windows" />);

    const copyButtons = screen.getAllByRole('button', { name: /复制命令/ });
    expect(copyButtons.length).toBeGreaterThan(0);
    expect(copyButtons[0].getAttribute('title')).toBeNull();

    fireEvent.click(copyButtons[0]);

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith('winget install Gyan.FFmpeg');
      expect(copyButtons[0].classList.contains('btn-copied')).toBe(true);
    });
  });

  it('opens official download page when clicking external link', async () => {
    render(<FfmpegInstallGuide isValid={false} onRecheck={vi.fn()} platform="windows" />);

    const downloadLink = screen.getByText('FFmpeg 官方下载页面');
    fireEvent.click(downloadLink);

    await waitFor(() => {
      expect(mockOpenUrl).toHaveBeenCalledWith('https://ffmpeg.org/download.html');
    });
  });

  it('handles recheck action and displays feedback banner', async () => {
    const onRecheck = vi.fn().mockResolvedValue(true);
    render(<FfmpegInstallGuide isValid={false} onRecheck={onRecheck} platform="windows" />);

    const recheckBtn = screen.getByRole('button', { name: /重新检测/ });
    fireEvent.click(recheckBtn);

    await waitFor(() => {
      expect(onRecheck).toHaveBeenCalledTimes(1);
      expect(screen.getByText('已成功检测到 FFmpeg！')).toBeDefined();
    });
  });

  it('displays failure banner when recheck returns false', async () => {
    const onRecheck = vi.fn().mockResolvedValue(false);
    render(<FfmpegInstallGuide isValid={false} onRecheck={onRecheck} platform="windows" />);

    const recheckBtn = screen.getByRole('button', { name: /重新检测/ });
    fireEvent.click(recheckBtn);

    await waitFor(() => {
      expect(onRecheck).toHaveBeenCalledTimes(1);
      expect(
        screen.getByText('仍未检测到 FFmpeg，请检查是否已安装并加入系统 PATH。')
      ).toBeDefined();
    });
  });

  it('is collapsed by default when isValid is true and expands on click', () => {
    render(<FfmpegInstallGuide isValid={true} onRecheck={vi.fn()} platform="windows" />);

    // Collapsed by default: commands shouldn't be visible
    expect(screen.queryByText('winget install Gyan.FFmpeg')).toBeNull();
    expect(screen.getByText(/FFmpeg 已就绪/)).toBeDefined();

    // Click header to expand
    const headerBtn = screen.getByRole('button', { name: /FFmpeg 已就绪/ });
    fireEvent.click(headerBtn);

    // Now expanded
    expect(screen.getByText('winget install Gyan.FFmpeg')).toBeDefined();
  });
});
