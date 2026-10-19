import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
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
    getModelCatalogSnapshot: vi.fn(),
    resolveAsrSelectedModelIdsFromSnapshot: vi.fn(),
    resolveScenarioSelectedModelIdsFromSnapshot: vi.fn(),
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
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  ask: vi.fn(),
  message: vi.fn(),
}));

describe('Settings search integration', () => {
  const onClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    useTranscriptStore.setState({ config: buildTestConfig() });
    vi.mocked(dashboardService.getFastSnapshot).mockResolvedValue(createEmptyDashboardSnapshot());
    vi.mocked(dashboardService.getDeepSnapshot).mockResolvedValue(createEmptyDashboardSnapshot());
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

  it('pressing Ctrl+F focuses search input and selects text', () => {
    render(<Settings isOpen={true} onClose={onClose} />);

    const searchInput = screen.getByPlaceholderText('搜索设置...') as HTMLInputElement;
    const focusSpy = vi.spyOn(searchInput, 'focus');
    const selectSpy = vi.spyOn(searchInput, 'select');

    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });

    expect(focusSpy).toHaveBeenCalled();
    expect(selectSpy).toHaveBeenCalled();
  });

  it('clicking a result calls navigateToTab and triggers element scrolling', async () => {
    const scrollIntoViewMock = vi.fn();
    const targetElement = document.createElement('div');
    targetElement.id = 'settings-language';
    targetElement.scrollIntoView = scrollIntoViewMock;
    document.body.appendChild(targetElement);

    try {
      render(<Settings isOpen={true} onClose={onClose} />);

      const searchInput = screen.getByPlaceholderText('搜索设置...');
      fireEvent.change(searchInput, { target: { value: '语言' } });

      const options = screen.getAllByRole('option');
      expect(options.length).toBeGreaterThan(0);

      // Click the first result (general-language)
      fireEvent.click(options[0]);

      // Element should be scrolled and class added
      await waitFor(() => {
        expect(scrollIntoViewMock).toHaveBeenCalled();
      });
      expect(targetElement.classList.contains('settings-item--highlighted')).toBe(true);
    } finally {
      document.body.removeChild(targetElement);
    }
  });

  it('navigates results with ArrowDown / ArrowUp and selects with Enter', async () => {
    const scrollIntoViewMock = vi.fn();
    const targetElement = document.createElement('div');
    targetElement.id = 'settings-language';
    targetElement.scrollIntoView = scrollIntoViewMock;
    document.body.appendChild(targetElement);

    try {
      render(<Settings isOpen={true} onClose={onClose} />);

      const searchInput = screen.getByPlaceholderText('搜索设置...');
      fireEvent.change(searchInput, { target: { value: '语言' } });

      const options = screen.getAllByRole('option');
      expect(options[0].getAttribute('data-active')).toBe('true');

      // Arrow down
      fireEvent.keyDown(searchInput, { key: 'ArrowDown', code: 'ArrowDown' });
      if (options.length > 1) {
        expect(options[1].getAttribute('data-active')).toBe('true');
      }

      // Arrow up back to 0
      fireEvent.keyDown(searchInput, { key: 'ArrowUp', code: 'ArrowUp' });
      expect(options[0].getAttribute('data-active')).toBe('true');

      // Enter selects active result
      fireEvent.keyDown(searchInput, { key: 'Enter', code: 'Enter' });

      await waitFor(() => {
        expect(scrollIntoViewMock).toHaveBeenCalled();
      });
    } finally {
      document.body.removeChild(targetElement);
    }
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
  it('does not trigger selection on Enter during IME composition', () => {
    const scrollIntoViewMock = vi.fn();
    const targetElement = document.createElement('div');
    targetElement.id = 'settings-language';
    targetElement.scrollIntoView = scrollIntoViewMock;
    document.body.appendChild(targetElement);

    try {
      render(<Settings isOpen={true} onClose={onClose} />);
      const searchInput = screen.getByPlaceholderText('搜索设置...') as HTMLInputElement;
      fireEvent.change(searchInput, { target: { value: '语言' } });

      const options = screen.getAllByRole('option');
      expect(options.length).toBeGreaterThan(0);

      fireEvent.keyDown(searchInput, {
        key: 'Enter',
        code: 'Enter',
        nativeEvent: { isComposing: true } as unknown as KeyboardEvent,
      });

      // Search query remains intact and selection was suppressed
      expect(searchInput.value).toBe('语言');
      expect(scrollIntoViewMock).not.toHaveBeenCalled();
      expect(targetElement.classList.contains('settings-item--highlighted')).toBe(false);
    } finally {
      document.body.removeChild(targetElement);
    }
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
});
