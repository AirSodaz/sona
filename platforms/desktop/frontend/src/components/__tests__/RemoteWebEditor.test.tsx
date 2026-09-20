import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { RemoteWebEditor } from '../RemoteWebEditor';

const mockChangeLanguage = vi.fn();

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => {
      return options?.defaultValue || _key;
    },
    i18n: {
      changeLanguage: mockChangeLanguage,
      language: 'en',
    },
  }),
}));

vi.mock('../../services/apiServerClient', () => ({
  apiServerClient: {
    getBaseUrl: vi.fn(() => 'http://127.0.0.1:14200'),
    getApiKey: vi.fn(() => ''),
    checkHealth: vi.fn().mockResolvedValue({ status: 'ok' }),
    getInfo: vi.fn().mockResolvedValue({
      models: ['test-model'],
      gpuAvailable: false,
    }),
    setBaseUrl: vi.fn(),
    setApiKey: vi.fn(),
    transcribe: vi.fn(),
    getJobStatus: vi.fn(),
    getAudioUrl: vi.fn(),
  },
}));

vi.mock('../transcript/TranscriptEditor', () => ({
  TranscriptEditor: () => <div data-testid="transcript-editor">Transcript Editor</div>,
}));

vi.mock('../AudioPlayer', () => ({
  AudioPlayer: () => <div data-testid="audio-player">Audio Player</div>,
}));

describe('RemoteWebEditor Language and Theme Controls', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    document.documentElement.removeAttribute('data-theme');

    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));
  });

  afterEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-theme');
  });

  it('renders language switcher pill with default System label and opens menu on click', async () => {
    render(<RemoteWebEditor />);

    const languageBtn = screen.getByTitle('Switch language');
    expect(languageBtn).toBeDefined();
    expect(languageBtn.textContent).toContain('System');

    fireEvent.click(languageBtn);

    const menuItems = screen
      .getAllByRole('button')
      .filter((b) =>
        ['System', 'English', '简体中文', '繁體中文', '日本語', '한국어'].some((lang) =>
          b.textContent?.includes(lang)
        )
      );
    expect(menuItems.length).toBeGreaterThanOrEqual(6);
  });

  it('persists selected language to localStorage and invokes i18n.changeLanguage', async () => {
    render(<RemoteWebEditor />);

    const languageBtn = screen.getByTitle('Switch language');
    fireEvent.click(languageBtn);

    const englishOption = screen.getByRole('button', { name: /English/i });
    fireEvent.click(englishOption);

    expect(localStorage.getItem('sona_web_language')).toBe('en');
    expect(mockChangeLanguage).toHaveBeenCalledWith('en');

    expect(screen.getByTitle('Switch language').textContent).toContain('English');
  });

  it('restores previously saved language preference from localStorage', async () => {
    localStorage.setItem('sona_web_language', 'ja');

    render(<RemoteWebEditor />);

    expect(screen.getByTitle('Switch language').textContent).toContain('日本語');
    expect(mockChangeLanguage).toHaveBeenCalledWith('ja');
  });

  it('renders theme switcher pill with default System label and opens theme menu', async () => {
    render(<RemoteWebEditor />);

    const themeBtn = screen.getByTitle('Toggle theme');
    expect(themeBtn).toBeDefined();
    expect(themeBtn.textContent).toContain('System');

    fireEvent.click(themeBtn);

    expect(screen.getByRole('button', { name: /Light/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /Dark/i })).toBeDefined();
  });

  it('applies dark theme, sets data-theme attribute on document root, and persists to localStorage', async () => {
    render(<RemoteWebEditor />);

    const themeBtn = screen.getByTitle('Toggle theme');
    fireEvent.click(themeBtn);

    const darkOption = screen.getByRole('button', { name: /Dark/i });
    fireEvent.click(darkOption);

    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    expect(localStorage.getItem('sona_web_theme')).toBe('dark');
    expect(screen.getByTitle('Toggle theme').textContent).toContain('Dark');
  });

  it('applies light theme and updates data-theme attribute on document root', async () => {
    render(<RemoteWebEditor />);

    const themeBtn = screen.getByTitle('Toggle theme');
    fireEvent.click(themeBtn);

    const lightOption = screen.getByRole('button', { name: /Light/i });
    fireEvent.click(lightOption);

    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    expect(localStorage.getItem('sona_web_theme')).toBe('light');
    expect(screen.getByTitle('Toggle theme').textContent).toContain('Light');
  });

  it('restores previously saved theme from localStorage on initial render', async () => {
    localStorage.setItem('sona_web_theme', 'dark');

    render(<RemoteWebEditor />);

    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    expect(screen.getByTitle('Toggle theme').textContent).toContain('Dark');
  });

  it('closes open menus when Escape key is pressed', async () => {
    render(<RemoteWebEditor />);

    const themeBtn = screen.getByTitle('Toggle theme');
    fireEvent.click(themeBtn);
    expect(screen.getByRole('button', { name: /Dark/i })).toBeDefined();

    fireEvent.keyDown(document, { key: 'Escape' });

    await waitFor(() => {
      expect(screen.queryByRole('button', { name: /Dark/i })).toBeNull();
    });
  });

  it('closes open menus when clicking outside', async () => {
    render(<RemoteWebEditor />);

    const languageBtn = screen.getByTitle('Switch language');
    fireEvent.click(languageBtn);
    expect(screen.getByRole('button', { name: /English/i })).toBeDefined();

    fireEvent.mouseDown(document.body);

    await waitFor(() => {
      expect(screen.queryByRole('button', { name: /English/i })).toBeNull();
    });
  });
});
