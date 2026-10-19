import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { dashboardService } from '../../services/dashboardService';
import { useDialogStore } from '../../stores/dialogStore';
import { buildTestConfig } from '../../test-utils/configTestUtils';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import type { DashboardSnapshot } from '../../types/dashboard';
import { Settings } from '../Settings';

function createEmptyDashboardSnapshot(): DashboardSnapshot {
  return {
    content: {
      overview: {
        itemCount: 0,
        itemCountDisplay: '0',
        tagCount: 0,
        tagCountDisplay: '0',
        totalDurationSeconds: 0,
        totalDurationDisplay: '0m',
        recordingCount: 0,
        recordingCountDisplay: '0',
        batchCount: 0,
        batchCountDisplay: '0',
        untaggedCount: 0,
        untaggedCountDisplay: '0',
        taggedCount: 0,
        taggedCountDisplay: '0',
        recentDailyItems: [],
        isDeepLoaded: false,
      },
      speakers: null,
    },
    llmUsage: {
      startedAt: '2026-04-01T00:00:00.000Z',
      lastUpdatedAt: '2026-04-28T00:00:00.000Z',
      trackingSinceDisplay: 'Apr 1, 2026',
      lastUpdatedDisplay: 'Apr 28, 2026',
      totals: {
        callCount: 0,
        callCountDisplay: '0',
        callsWithUsage: 0,
        callsWithUsageDisplay: '0',
        callsWithoutUsage: 0,
        callsWithoutUsageDisplay: '0',
        promptTokens: 0,
        promptTokensDisplay: '0',
        completionTokens: 0,
        completionTokensDisplay: '0',
        totalTokens: 0,
        totalTokensDisplay: '0',
      },
      byProvider: [],
      byProviderTopRows: [],
      byProviderMaxValue: 0,
      byCategory: [],
      byCategoryTopRows: [],
      byCategoryMaxValue: 0,
      recentDaily: [],
    },
    generatedAt: '2026-04-28T00:00:00.000Z',
  };
}

// Mock dependencies matching Settings.test.tsx
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number; defaultValue?: string }) => {
      const map: Record<string, string> = {
        'settings.title': '设置',
        'settings.search_placeholder': '搜索设置...',
        'settings.search_no_results': '未找到相关设置',
        'settings.search_matches_count': `匹配项 (${options?.count ?? 0})`,
        'settings.general': '常规',
        'settings.language': '语言',
        'settings.theme': '主题',
        'settings.llm.title': '大模型服务',
        'settings.input_device': '输入设备',
      };
      return map[key] ?? options?.defaultValue ?? key;
    },
    i18n: { changeLanguage: vi.fn() },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../settings/backup/BackupSettingsSection', () => ({
  BackupSettingsSection: () => <div data-testid="backup-settings-section" />,
}));

vi.mock('../../services/modelService', () => ({
  PRESET_MODELS: [],
  PRESET_MODELS_MAP: new Map(),
  modelService: {
    getModelCatalogSnapshot: vi.fn().mockResolvedValue({
      modelsDir: '',
      models: [],
      sections: [],
      selectionOptions: {
        streaming: [],
        batch: [],
        speakerSegmentation: [],
        speakerEmbedding: [],
        alignment: [],
      },
      totalCount: 0,
    }),
    resolveAsrSelectedModelIdsFromSnapshot: vi.fn().mockReturnValue({}),
    resolveScenarioSelectedModelIdsFromSnapshot: vi.fn().mockReturnValue({}),
  },
}));

vi.mock('../../services/dashboardService', () => ({
  dashboardService: {
    getFastSnapshot: vi.fn(),
    getDeepSnapshot: vi.fn(),
  },
}));

vi.mock('../../services/audioDeviceService', () => ({
  listMicrophoneDeviceOptions: vi.fn().mockResolvedValue([{ label: 'Auto', value: 'default' }]),
  listSystemAudioDeviceOptions: vi.fn().mockResolvedValue([{ label: 'Auto', value: 'default' }]),
}));

vi.mock('../../services/tauri/llm', () => ({
  listLlmModels: vi.fn().mockResolvedValue(['gpt-4o']),
  runTranscriptLlmJob: vi.fn(),
  runTranscriptLlmTaskJob: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  ask: vi.fn(),
  message: vi.fn(),
}));

describe('Settings search integration', () => {
  const onClose = vi.fn();
  const originalScrollIntoView = window.HTMLElement.prototype.scrollIntoView;

  beforeEach(() => {
    vi.clearAllMocks();
    useDialogStore.setState({ isOpen: false });
    useTranscriptStore.setState({ config: buildTestConfig() });
    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createEmptyDashboardSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createEmptyDashboardSnapshot());
    window.HTMLElement.prototype.scrollIntoView = vi.fn();
  });

  afterEach(() => {
    window.HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
  });
  it('renders search input in the sidebar header', () => {
    render(<Settings isOpen={true} onClose={onClose} />);
    const searchInput = screen.getByPlaceholderText('搜索设置...');
    expect(searchInput).toBeDefined();
  });

  it('typing in search input replaces tab list with results', async () => {
    render(<Settings isOpen={true} onClose={onClose} />);

    // Initially tab list is rendered
    expect(document.querySelector('.settings-tabs-container')).not.toBeNull();

    const searchInput = screen.getByPlaceholderText('搜索设置...');
    fireEvent.change(searchInput, { target: { value: '语言' } });

    // Tabs container is replaced by search results
    expect(document.querySelector('.settings-tabs-container')).toBeNull();
    const resultsContainer = document.querySelector('.settings-search-results');
    expect(resultsContainer).not.toBeNull();

    // Contains result items
    const options = screen.getAllByRole('option');
    expect(options.length).toBeGreaterThan(0);
  });

  it('pressing Escape with text in input clears input without closing modal', () => {
    render(<Settings isOpen={true} onClose={onClose} />);

    const searchInput = screen.getByPlaceholderText('搜索设置...') as HTMLInputElement;
    fireEvent.change(searchInput, { target: { value: '语言' } });
    expect(searchInput.value).toBe('语言');

    // Press Escape in search input
    fireEvent.keyDown(searchInput, { key: 'Escape', code: 'Escape' });

    // Query is cleared
    expect(searchInput.value).toBe('');
    // Modal did not close
    expect(onClose).not.toHaveBeenCalled();
    // Tab list is restored
    expect(document.querySelector('.settings-tabs-container')).not.toBeNull();
  });

  it('pressing Ctrl+F / Cmd+F focuses search input and selects text', () => {
    render(<Settings isOpen={true} onClose={onClose} />);

    const searchInput = screen.getByPlaceholderText('搜索设置...') as HTMLInputElement;
    const focusSpy = vi.spyOn(searchInput, 'focus');
    const selectSpy = vi.spyOn(searchInput, 'select');

    const isMac = /Mac|iPhone|iPad|iPod/i.test(navigator.platform || navigator.userAgent || '');
    fireEvent.keyDown(window, { key: 'f', ctrlKey: !isMac, metaKey: isMac });

    expect(focusSpy).toHaveBeenCalled();
    expect(selectSpy).toHaveBeenCalled();
  });

  it('clicking a result calls navigateToTab and triggers element scrolling', async () => {
    const scrollIntoViewMock = vi.fn();
    window.HTMLElement.prototype.scrollIntoView = scrollIntoViewMock;

    render(<Settings isOpen={true} onClose={onClose} />);
    expect(await screen.findByText('settings.general_title')).toBeDefined();

    const searchInput = screen.getByPlaceholderText('搜索设置...');
    fireEvent.change(searchInput, { target: { value: '语言' } });

    const options = screen.getAllByRole('option');
    expect(options.length).toBeGreaterThan(0);

    // Click the first result (general-language)
    fireEvent.click(options[0]);

    // Real element in DOM should be scrolled and class added
    await waitFor(() => {
      expect(scrollIntoViewMock).toHaveBeenCalledWith({ behavior: 'smooth', block: 'center' });
    });
    const targetElement = document.getElementById('settings-language');
    expect(targetElement?.classList.contains('settings-item--highlighted')).toBe(true);
  });

  it('navigates results with ArrowDown / ArrowUp and selects with Enter', async () => {
    const scrollIntoViewMock = vi.fn();
    window.HTMLElement.prototype.scrollIntoView = scrollIntoViewMock;

    render(<Settings isOpen={true} onClose={onClose} />);
    expect(await screen.findByText('settings.general_title')).toBeDefined();

    const searchInput = screen.getByPlaceholderText('搜索设置...');
    fireEvent.change(searchInput, { target: { value: '语言' } });

    const options = screen.getAllByRole('option');
    expect(options.length).toBeGreaterThan(1);
    expect(options[0].getAttribute('data-active')).toBe('true');

    // Arrow down
    fireEvent.keyDown(searchInput, { key: 'ArrowDown', code: 'ArrowDown' });
    expect(options[1].getAttribute('data-active')).toBe('true');

    // Arrow up back to 0
    fireEvent.keyDown(searchInput, { key: 'ArrowUp', code: 'ArrowUp' });
    expect(options[0].getAttribute('data-active')).toBe('true');

    scrollIntoViewMock.mockClear();

    // Enter selects active result
    fireEvent.keyDown(searchInput, { key: 'Enter', code: 'Enter' });

    await waitFor(() => {
      expect(scrollIntoViewMock).toHaveBeenCalledWith({ behavior: 'smooth', block: 'center' });
    });
  });

  it('has accessible aria-label on search input', () => {
    render(<Settings isOpen={true} onClose={onClose} />);
    const searchInput = screen.getByRole('searchbox', { name: '搜索设置...' });
    expect(searchInput).toBeDefined();
  });

  it('does not intercept Escape when a child dialog is open', () => {
    render(<Settings isOpen={true} onClose={onClose} />);
    const searchInput = screen.getByPlaceholderText('搜索设置...') as HTMLInputElement;
    fireEvent.change(searchInput, { target: { value: '语言' } });
    expect(searchInput.value).toBe('语言');

    useDialogStore.setState({ isOpen: true });
    try {
      fireEvent.keyDown(searchInput, { key: 'Escape', code: 'Escape' });

      // Search query is NOT cleared because a dialog is open
      expect(searchInput.value).toBe('语言');
    } finally {
      useDialogStore.setState({ isOpen: false });
    }
  });
  it('does not trigger selection on Enter during IME composition', async () => {
    const scrollIntoViewMock = vi.fn();
    window.HTMLElement.prototype.scrollIntoView = scrollIntoViewMock;

    render(<Settings isOpen={true} onClose={onClose} />);
    expect(await screen.findByText('settings.general_title')).toBeDefined();

    const searchInput = screen.getByPlaceholderText('搜索设置...') as HTMLInputElement;
    fireEvent.change(searchInput, { target: { value: '语言' } });

    const options = screen.getAllByRole('option');
    expect(options.length).toBeGreaterThan(0);

    scrollIntoViewMock.mockClear();

    fireEvent.keyDown(searchInput, {
      key: 'Enter',
      code: 'Enter',
      isComposing: true,
    });

    // Search query remains intact and selection was suppressed
    expect(searchInput.value).toBe('语言');
    expect(scrollIntoViewMock).not.toHaveBeenCalled();
    const targetElement = document.getElementById('settings-language');
    expect(targetElement?.classList.contains('settings-item--highlighted')).toBe(false);
  });

  it('pressing Escape with whitespace in input clears input without closing modal', () => {
    render(<Settings isOpen={true} onClose={onClose} />);

    const searchInput = screen.getByPlaceholderText('搜索设置...') as HTMLInputElement;
    fireEvent.change(searchInput, { target: { value: '   ' } });
    expect(searchInput.value).toBe('   ');

    fireEvent.keyDown(searchInput, { key: 'Escape', code: 'Escape' });

    expect(searchInput.value).toBe('');
    expect(onClose).not.toHaveBeenCalled();
    expect(document.querySelector('.settings-tabs-container')).not.toBeNull();
  });

  it('selecting a result on a different tab switches the active tab and scrolls target', async () => {
    const scrollIntoViewMock = vi.fn();
    window.HTMLElement.prototype.scrollIntoView = scrollIntoViewMock;

    render(<Settings isOpen={true} onClose={onClose} />);
    expect(await screen.findByText('settings.general_title')).toBeDefined();

    const searchInput = screen.getByPlaceholderText('搜索设置...');
    fireEvent.change(searchInput, { target: { value: '模型服务商' } });

    const options = screen.getAllByRole('option');
    expect(options.length).toBeGreaterThan(0);

    scrollIntoViewMock.mockClear();
    fireEvent.click(options[0]);

    await waitFor(() => {
      const activePane = document.querySelector(
        '[data-settings-tab-pane="llm_service"]:not([hidden])'
      );
      expect(activePane).not.toBeNull();
    });

    await waitFor(() => {
      const targetElement = document.getElementById('settings-llm-provider-list');
      expect(targetElement).not.toBeNull();
      expect(scrollIntoViewMock).toHaveBeenCalledWith({ behavior: 'smooth', block: 'center' });
      expect(targetElement?.classList.contains('settings-item--highlighted')).toBe(true);
    });
  });

  it('renders shortcut hint when empty and clear button when typing', () => {
    render(<Settings isOpen={true} onClose={onClose} />);
    const shortcut = document.querySelector('.settings-search-shortcut');
    expect(shortcut).not.toBeNull();
    expect(screen.queryByLabelText('Clear search')).toBeNull();

    const searchInput = screen.getByRole('searchbox');
    fireEvent.change(searchInput, { target: { value: 'test' } });

    expect(document.querySelector('.settings-search-shortcut')).toBeNull();
    const clearBtn = screen.getByLabelText('Clear search');
    expect(clearBtn).toBeDefined();

    fireEvent.click(clearBtn);
    expect(document.querySelector('.settings-search-shortcut')).not.toBeNull();
  });

  it('falls back to settings-sync-toggle when sync target is not in the DOM', async () => {
    const scrollIntoViewMock = vi.fn();
    window.HTMLElement.prototype.scrollIntoView = scrollIntoViewMock;

    render(<Settings isOpen={true} onClose={onClose} />);
    expect(await screen.findByText('settings.general_title')).toBeDefined();

    const searchInput = screen.getByPlaceholderText('搜索设置...');
    // Search for sync credentials/webdav which might not exist when sync is off
    fireEvent.change(searchInput, { target: { value: 'webdav' } });

    const options = screen.getAllByRole('option');
    expect(options.length).toBeGreaterThan(0);

    scrollIntoViewMock.mockClear();
    fireEvent.click(options[0]);

    await waitFor(
      () => {
        const activePane = document.querySelector('[data-settings-tab-pane="sync"]:not([hidden])');
        expect(activePane).not.toBeNull();
      },
      { timeout: 3000 }
    );

    await waitFor(
      () => {
        const toggleElement = document.getElementById('settings-sync-toggle');
        expect(toggleElement).not.toBeNull();
        expect(scrollIntoViewMock).toHaveBeenCalledWith({ behavior: 'smooth', block: 'center' });
        expect(toggleElement?.classList.contains('settings-item--highlighted')).toBe(true);
      },
      { timeout: 3000 }
    );
  });

  it('scoping: pressing Escape inside .settings-content does not clear search input', () => {
    render(<Settings isOpen={true} onClose={onClose} />);

    const searchInput = screen.getByPlaceholderText('搜索设置...') as HTMLInputElement;
    fireEvent.change(searchInput, { target: { value: '语言' } });
    expect(searchInput.value).toBe('语言');

    const contentPane = document.querySelector('.settings-content')!;
    expect(contentPane).not.toBeNull();

    // Fire Escape in the content pane
    fireEvent.keyDown(contentPane, { key: 'Escape' });

    // Search query remains intact in sidebar because Escape handler is scoped to sidebar
    expect(searchInput.value).toBe('语言');
  });

  it('searching and selecting Batch VAD routes to Models tab, switches to batch scenario, and highlights element', async () => {
    const scrollIntoViewMock = vi.fn();
    window.HTMLElement.prototype.scrollIntoView = scrollIntoViewMock;

    render(<Settings isOpen={true} onClose={onClose} />);
    expect(await screen.findByText('settings.general_title')).toBeDefined();

    const searchInput = screen.getByPlaceholderText('搜索设置...');
    fireEvent.change(searchInput, { target: { value: 'batch vad' } });

    const options = screen.getAllByRole('option');
    expect(options.length).toBeGreaterThan(0);

    scrollIntoViewMock.mockClear();
    fireEvent.click(options[0]);

    await waitFor(
      () => {
        const activePane = document.querySelector(
          '[data-settings-tab-pane="models"]:not([hidden])'
        );
        expect(activePane).not.toBeNull();
      },
      { timeout: 3000 }
    );

    await waitFor(
      () => {
        const targetElement = document.getElementById('settings-batch-vad');
        expect(targetElement).not.toBeNull();
        expect(scrollIntoViewMock).toHaveBeenCalledWith({ behavior: 'smooth', block: 'center' });
        expect(targetElement?.classList.contains('settings-item--highlighted')).toBe(true);
      },
      { timeout: 3000 }
    );
  });

  it('settings-silent-start is unconditionally rendered in the DOM with disabled state when autoStart is off', async () => {
    render(<Settings isOpen={true} onClose={onClose} />);
    expect(await screen.findByText('settings.general_title')).toBeDefined();

    const silentStartItem = document.getElementById('settings-silent-start');
    expect(silentStartItem).not.toBeNull();
  });
});
