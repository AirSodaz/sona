import { readFileSync } from 'node:fs';
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { HistoryItem } from '../HistoryItem';
import { useProjectStore } from '../../../stores/projectStore';

const historyLayoutsCss = readFileSync(
  'src/styles/history-layouts.css',
  'utf8',
);

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      key: string,
      options?: { defaultValue?: string } & Record<string, unknown>,
    ) => {
      if (typeof options?.defaultValue === 'string') {
        return options.defaultValue.replace(/\{\{(\w+)\}\}/g, (_: string, variable: string) => String(options?.[variable] ?? ''));
      }
      return key;
    },
    i18n: { language: 'en' },
  }),
}));

describe('HistoryItem', () => {
  const item = {
    id: 'hist-1',
    title: 'Client Call',
    timestamp: Date.now(),
    duration: 185,
    audioPath: 'audio.wav',
    transcriptPath: 'hist-1.json',
    previewText: 'Quarterly planning follow-up',
    searchContent: 'Quarterly planning follow-up',
    type: 'recording',
    projectId: 'project-1',
  } as const;

  beforeEach(() => {
    useProjectStore.setState({
      ...useProjectStore.getState(),
      projects: [
        {
          id: 'project-1',
          name: 'Alpha',
          description: 'Project alpha',
          icon: '',
          createdAt: 1,
          updatedAt: 1,
          defaults: {
            summaryTemplateId: 'general',
            translationLanguage: 'zh',
            polishPresetId: 'general',
            exportFileNamePrefix: '',
            enabledTextReplacementSetIds: [],
            enabledHotwordSetIds: [],
            enabledPolishKeywordSetIds: [],
            enabledSpeakerProfileIds: [],
          },
        },
      ],
    });
  });

  it('keeps the delete action accessible without hijacking the main open action', () => {
    const onLoad = vi.fn();
    const onDelete = vi.fn();

    render(
      <HistoryItem
        item={item}
        onLoad={onLoad}
        onDelete={onDelete}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Delete Client Call' }));

    expect(onDelete).toHaveBeenCalledTimes(1);
    expect(onDelete).toHaveBeenCalledWith('hist-1');
    expect(onLoad).not.toHaveBeenCalled();
  });

  it('hides the delete action in selection mode and toggles selection from the row content', () => {
    const onLoad = vi.fn();
    const onDelete = vi.fn();
    const onToggleSelection = vi.fn();

    render(
      <HistoryItem
        item={item}
        onLoad={onLoad}
        onDelete={onDelete}
        isSelectionMode
        onToggleSelection={onToggleSelection}
      />,
    );

    expect(screen.queryByRole('button', { name: 'Delete Client Call' })).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Load Client Call' }));

    expect(onToggleSelection).toHaveBeenCalledWith('hist-1');
    expect(onLoad).not.toHaveBeenCalled();
    expect(onDelete).not.toHaveBeenCalled();
  });

  it('requests a pointer context menu without opening the history item', () => {
    const onLoad = vi.fn();
    const onOpenContextMenu = vi.fn();

    render(
      <HistoryItem
        item={item}
        onLoad={onLoad}
        onDelete={vi.fn()}
        onOpenContextMenu={onOpenContextMenu}
        isContextMenuOpen
      />,
    );

    const historyItem = screen.getByRole('listitem');
    const contentButton = screen.getByRole('button', { name: 'Load Client Call' });
    fireEvent.contextMenu(historyItem, {
      button: 2,
      clientX: 140,
      clientY: 180,
    });

    expect(onOpenContextMenu).toHaveBeenCalledWith('hist-1', {
      anchor: contentButton,
      point: { x: 140, y: 180 },
      invocation: 'pointer',
    });
    expect(historyItem.classList.contains('context-menu-active')).toBe(true);
    expect(onLoad).not.toHaveBeenCalled();
  });

  it.each([
    { key: 'F10', shiftKey: true },
    { key: 'ContextMenu', shiftKey: false },
  ])('requests a keyboard context menu for $key', ({ key, shiftKey }) => {
    const onOpenContextMenu = vi.fn();

    render(
      <HistoryItem
        item={item}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        onOpenContextMenu={onOpenContextMenu}
      />,
    );

    const trigger = screen.getByRole('button', { name: 'Load Client Call' });
    vi.spyOn(trigger, 'getBoundingClientRect').mockReturnValue({
      x: 32,
      y: 48,
      width: 240,
      height: 80,
      top: 48,
      right: 272,
      bottom: 128,
      left: 32,
      toJSON: () => ({}),
    });

    fireEvent.keyDown(trigger, { key, shiftKey });

    expect(onOpenContextMenu).toHaveBeenCalledWith('hist-1', {
      anchor: trigger,
      point: { x: 44, y: 60 },
      invocation: 'keyboard',
    });
  });

  it('keeps a load-locked item focusable for keyboard context menu requests', () => {
    const onOpenContextMenu = vi.fn();

    render(
      <HistoryItem
        item={item}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        onOpenContextMenu={onOpenContextMenu}
        isLoadDisabled
      />,
    );

    const trigger = screen.getByRole('button', { name: 'Load Client Call' }) as HTMLButtonElement;
    expect(trigger.disabled).toBe(false);
    expect(trigger.getAttribute('aria-disabled')).toBe('true');

    fireEvent.keyDown(trigger, { key: 'F10', shiftKey: true });

    expect(onOpenContextMenu).toHaveBeenCalledWith('hist-1', expect.objectContaining({
      anchor: trigger,
      invocation: 'keyboard',
    }));
  });

  it('does not request an item context menu while selection mode is active', () => {
    const onOpenContextMenu = vi.fn();

    render(
      <HistoryItem
        item={item}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        onOpenContextMenu={onOpenContextMenu}
        isSelectionMode
      />,
    );

    const historyItem = screen.getByRole('listitem');
    fireEvent.contextMenu(historyItem, { clientX: 40, clientY: 50 });
    fireEvent.keyDown(screen.getByRole('button', { name: 'Load Client Call' }), {
      key: 'F10',
      shiftKey: true,
    });

    expect(onOpenContextMenu).not.toHaveBeenCalled();
  });

  it('renders a search snippet with highlight for list layouts', () => {
    const { container } = render(
      <HistoryItem
        item={item}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        searchQuery="roadmap"
        searchSnippet={{
          text: 'Quarterly roadmap follow-up',
          highlightStart: 10,
          highlightEnd: 17,
        }}
      />,
    );

    expect(container.querySelector('.history-item-preview')?.textContent).toContain('Quarterly roadmap follow-up');
    expect(screen.getByText('roadmap').tagName).toBe('MARK');
  });

  it('keeps the project badge visible by default and hides it when requested', () => {
    const { rerender } = render(
      <HistoryItem
        item={item}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText('Alpha')).toBeDefined();

    rerender(
      <HistoryItem
        item={item}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        showProjectBadge={false}
      />,
    );

    expect(screen.queryByText('Alpha')).toBeNull();
  });

  it('shows the snippet as a compact secondary line in table layout', () => {
    const { container } = render(
      <HistoryItem
        item={item}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        searchQuery="planning"
        searchSnippet={{
          text: 'Quarterly planning follow-up',
          highlightStart: 10,
          highlightEnd: 18,
        }}
        layout="table"
        isKeyboardActive
      />,
    );

    expect(container.querySelector('.history-item')?.classList.contains('keyboard-active')).toBe(true);
    expect(container.querySelector('.history-item-preview--table')).not.toBeNull();
    expect(container.querySelector('.history-item--table .history-item-header .history-item-preview--table')).not.toBeNull();
    expect(screen.getByText('planning').tagName).toBe('MARK');
  });

  it('keeps table row dividers active in virtualized table rows', () => {
    expect(historyLayoutsCss).toMatch(/--projects-table-row-divider:\s*color-mix\(/);
    expect(historyLayoutsCss).toMatch(
      /\.history-item--table\s*{[^}]*border-bottom:\s*1px solid var\(--projects-table-row-divider\);/s,
    );
    expect(historyLayoutsCss).not.toMatch(
      /\.history-item--table:last-child\s*{[^}]*border-bottom:\s*none;/s,
    );
  });

  it('removes the draft badge when the same live recording history item completes', () => {
    const { rerender } = render(
      <HistoryItem
        item={{
          ...item,
          status: 'draft',
          draftSource: 'live_record',
        }}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText('Draft')).toBeDefined();

    rerender(
      <HistoryItem
        item={{
          ...item,
          status: 'complete',
          draftSource: undefined,
          previewText: 'Finished transcript',
        }}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.queryByText('Draft')).toBeNull();
  });
});
