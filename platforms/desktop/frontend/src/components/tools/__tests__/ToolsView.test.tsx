import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as Platform from '../../../platform';
import { useConfigStore } from '../../../stores/configStore';
import { useToolsStore } from '../../../stores/toolsStore';
import { useTranscriptRuntimeStore } from '../../../stores/transcriptRuntimeStore';
import { buildTestConfig } from '../../../test-utils/configTestUtils';
import { ToolsView } from '../ToolsView';

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
vi.mock('../../../platform', async (importOriginal) => {
  const actual = await importOriginal<typeof Platform>();
  return {
    ...actual,
    useTransport: () => ({
      invoke: vi.fn().mockResolvedValue({}),
    }),
  };
});

vi.mock('../../../services/voiceTypingReadiness', () => ({
  resolveVoiceTypingReadinessSnapshot: () => ({
    state: 'ready',
    shortcutConfigured: true,
    liveModelConfigured: true,
    requiresVad: false,
    vadConfigured: true,
    shortcutRegistration: 'ready',
    warmup: 'ready',
    inputDeviceState: 'ready',
    runtimeState: 'ready',
    lastErrorSource: null,
    lastErrorMessage: null,
  }),
}));

describe('ToolsView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useToolsStore.setState({ activeTool: 'subtitles' });
    useTranscriptRuntimeStore.setState({ isCaptionMode: false });
    useConfigStore.setState({
      config: buildTestConfig({
        voiceTypingEnabled: false,
        httpServerEnabled: false,
        mcpEnabled: false,
      }),
    });
  });

  afterEach(() => {
    cleanup();
  });

  it('renders sidebar with 4 tools and subtitle panel by default', () => {
    const { container } = render(<ToolsView />);

    // Check dual-panel alignment classes
    const workbench = screen.getByRole('region', { name: 'Tools Workbench' });
    expect(workbench.classList.contains('tools-workbench')).toBe(true);
    expect(workbench.classList.contains('panel-container')).toBe(true);

    const sidebar = container.querySelector('.tools-sidebar');
    expect(sidebar).not.toBeNull();
    expect(sidebar?.classList.contains('panel-left')).toBe(true);

    const detailPane = container.querySelector('.tools-detail-pane');
    expect(detailPane).not.toBeNull();
    expect(detailPane?.classList.contains('panel-right')).toBe(true);

    // Check sidebar tabs
    const sidebarTabs = screen.getAllByRole('tab');
    expect(sidebarTabs).toHaveLength(4);
    expect(sidebarTabs[0].textContent).toContain('Live Subtitles');
    expect(sidebarTabs[1].textContent).toContain('Voice Typing');
    expect(sidebarTabs[2].textContent).toContain('API Server');
    expect(sidebarTabs[3].textContent).toContain('MCP Agents');
    // Check default subtitles panel
    expect(document.getElementById('tools-panel-subtitles')).toBeDefined();
  });

  it('switches to voice typing panel when clicking voice typing in sidebar', () => {
    render(<ToolsView />);

    const sidebarTabs = screen.getAllByRole('tab');
    fireEvent.click(sidebarTabs[1]); // voice typing is index 1

    expect(useToolsStore.getState().activeTool).toBe('voice_typing');
    expect(document.getElementById('tools-panel-voice-typing')).toBeDefined();
  });

  it('toggles caption mode when clicking caption switch in subtitles panel', () => {
    render(<ToolsView />);

    expect(useTranscriptRuntimeStore.getState().isCaptionMode).toBe(false);

    // Find the caption mode switch in header
    const captionSwitch = screen.getByRole('switch', { name: 'Live Caption' });
    fireEvent.click(captionSwitch);

    expect(useTranscriptRuntimeStore.getState().isCaptionMode).toBe(true);
  });

  it('resets scroll position to top when active tool changes', () => {
    const { container } = render(<ToolsView />);
    const detailPane = container.querySelector('.tools-detail-pane') as HTMLElement;
    expect(detailPane).not.toBeNull();

    // Simulate user having scrolled down
    detailPane.scrollTop = 350;

    const sidebarTabs = screen.getAllByRole('tab');
    fireEvent.click(sidebarTabs[1]); // switch to voice typing

    expect(detailPane.scrollTop).toBe(0);
  });
});
