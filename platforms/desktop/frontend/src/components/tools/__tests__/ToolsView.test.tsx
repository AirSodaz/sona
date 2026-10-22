import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as Platform from '../../../platform';
import { useConfigStore } from '../../../stores/configStore';
import { useToolsStore } from '../../../stores/toolsStore';
import { useTranscriptRuntimeStore } from '../../../stores/transcriptRuntimeStore';
import { buildTestConfig } from '../../../test-utils/configTestUtils';
import { ToolsSidebar } from '../ToolsSidebar';
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
vi.mock('../../../services/tauri/invoke', () => ({
  invokeTauri: vi.fn().mockResolvedValue({}),
}));

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
    const workbench = screen.getByRole('region', { name: 'Tools' });
    expect(workbench.classList.contains('tools-workbench')).toBe(true);
    expect(workbench.classList.contains('panel-container')).toBe(true);
    expect(workbench.hasAttribute('hidden')).toBe(false);

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
    expect(sidebarTabs[0].getAttribute('id')).toBe('tools-tab-subtitles');
    expect(sidebarTabs[0].getAttribute('aria-controls')).toBe('tools-panel-subtitles');
    expect(sidebarTabs[0].getAttribute('tabindex')).toBe('0');

    expect(sidebarTabs[1].textContent).toContain('Voice Typing');
    expect(sidebarTabs[1].getAttribute('id')).toBe('tools-tab-voice_typing');
    expect(sidebarTabs[1].getAttribute('aria-controls')).toBe('tools-panel-voice_typing');
    expect(sidebarTabs[1].getAttribute('tabindex')).toBe('-1');

    expect(sidebarTabs[2].textContent).toContain('API Server');
    expect(sidebarTabs[2].getAttribute('id')).toBe('tools-tab-api_server');
    expect(sidebarTabs[2].getAttribute('aria-controls')).toBe('tools-panel-api_server');
    expect(sidebarTabs[2].getAttribute('tabindex')).toBe('-1');

    expect(sidebarTabs[3].textContent).toContain('MCP Agents');
    expect(sidebarTabs[3].getAttribute('id')).toBe('tools-tab-mcp');
    expect(sidebarTabs[3].getAttribute('aria-controls')).toBe('tools-panel-mcp');
    expect(sidebarTabs[3].getAttribute('tabindex')).toBe('-1');

    // Check default subtitles panel is mounted and unselected panels are unmounted
    expect(document.getElementById('tools-panel-subtitles')).not.toBeNull();
    expect(document.getElementById('tools-panel-voice_typing')).toBeNull();
    expect(document.getElementById('tools-panel-api_server')).toBeNull();
    expect(document.getElementById('tools-panel-mcp')).toBeNull();
  });

  it('switches to voice typing panel when clicking voice typing in sidebar', () => {
    render(<ToolsView />);

    const sidebarTabs = screen.getAllByRole('tab');
    fireEvent.click(sidebarTabs[1]); // voice typing is index 1

    expect(useToolsStore.getState().activeTool).toBe('voice_typing');
    expect(document.getElementById('tools-panel-voice_typing')).not.toBeNull();
    expect(document.getElementById('tools-panel-subtitles')).toBeNull();
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

  it('sets hidden and aria-hidden attributes when isActive is false', () => {
    const { container } = render(<ToolsView isActive={false} />);
    const workbench = container.querySelector('.tools-workbench') as HTMLElement;
    expect(workbench).not.toBeNull();
    expect(workbench.hasAttribute('hidden')).toBe(true);
    expect(workbench.getAttribute('aria-hidden')).toBe('true');
  });

  it('supports roving focus with arrow keys, Home, and End in sidebar tablist', () => {
    render(<ToolsSidebar />);
    const tablist = screen.getByRole('tablist');
    const sidebarTabs = screen.getAllByRole('tab');

    // Initial focus on subtitles (tab 0)
    sidebarTabs[0].focus();
    expect(useToolsStore.getState().activeTool).toBe('subtitles');

    // ArrowDown -> voice_typing (tab 1)
    fireEvent.keyDown(tablist, { key: 'ArrowDown' });
    expect(useToolsStore.getState().activeTool).toBe('voice_typing');

    // ArrowDown -> api_server (tab 2)
    fireEvent.keyDown(tablist, { key: 'ArrowDown' });
    expect(useToolsStore.getState().activeTool).toBe('api_server');

    // End -> mcp (tab 3)
    fireEvent.keyDown(tablist, { key: 'End' });
    expect(useToolsStore.getState().activeTool).toBe('mcp');

    // ArrowDown wraps -> subtitles (tab 0)
    fireEvent.keyDown(tablist, { key: 'ArrowDown' });
    expect(useToolsStore.getState().activeTool).toBe('subtitles');

    // ArrowUp wraps -> mcp (tab 3)
    fireEvent.keyDown(tablist, { key: 'ArrowUp' });
    expect(useToolsStore.getState().activeTool).toBe('mcp');

    // Home -> subtitles (tab 0)
    fireEvent.keyDown(tablist, { key: 'Home' });
    expect(useToolsStore.getState().activeTool).toBe('subtitles');
  });
});
