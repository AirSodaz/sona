import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useToolsStore } from '../../../stores/toolsStore';
import { useTranscriptRuntimeStore } from '../../../stores/transcriptRuntimeStore';
import { SettingsMigratedBanner } from '../SettingsMigratedBanner';

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

describe('SettingsMigratedBanner', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useToolsStore.setState({ activeTool: 'subtitles' });
    useTranscriptRuntimeStore.setState({ mode: 'live' });
  });

  afterEach(() => {
    cleanup();
  });

  it('renders migration notice for subtitles and navigates to tools tab', () => {
    const onClose = vi.fn();
    render(<SettingsMigratedBanner tool="subtitles" onClose={onClose} />);

    expect(screen.getAllByText('Live Subtitles').length).toBeGreaterThan(0);
    expect(
      screen.getByText(
        'This setting has been moved to the homepage Tools workbench for faster control, configuration, and monitoring.'
      )
    ).toBeDefined();

    const goToToolsBtn = screen.getByRole('button', { name: /Go to Tools/i });
    fireEvent.click(goToToolsBtn);

    expect(onClose).toHaveBeenCalled();
    expect(useToolsStore.getState().activeTool).toBe('subtitles');
    expect(useTranscriptRuntimeStore.getState().mode).toBe('tools');
  });

  it('renders migration notice for api_server and navigates to tools tab', () => {
    const onClose = vi.fn();
    render(<SettingsMigratedBanner tool="api_server" onClose={onClose} />);

    expect(screen.getAllByText('API Server').length).toBeGreaterThan(0);

    const goToToolsBtn = screen.getByRole('button', { name: /Go to Tools/i });
    fireEvent.click(goToToolsBtn);

    expect(onClose).toHaveBeenCalled();
    expect(useToolsStore.getState().activeTool).toBe('api_server');
    expect(useTranscriptRuntimeStore.getState().mode).toBe('tools');
  });

  it('renders migration notice for mcp and navigates to tools tab', () => {
    const onClose = vi.fn();
    render(<SettingsMigratedBanner tool="mcp" onClose={onClose} />);

    expect(screen.getAllByText('MCP Agents').length).toBeGreaterThan(0);

    const goToToolsBtn = screen.getByRole('button', { name: /Go to Tools/i });
    fireEvent.click(goToToolsBtn);

    expect(onClose).toHaveBeenCalled();
    expect(useToolsStore.getState().activeTool).toBe('mcp');
    expect(useTranscriptRuntimeStore.getState().mode).toBe('tools');
  });
});
