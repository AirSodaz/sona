import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { HistoryItem } from '../HistoryItem';
import { useProjectStore } from '../../../stores/projectStore';

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
    expect(onDelete.mock.calls[0][1]).toBe('hist-1');
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
    expect(screen.getByText('planning').tagName).toBe('MARK');
  });
});
