import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createStore } from 'zustand/vanilla';
import { useTranscriptStore } from '../../../test-utils/transcriptStoreTestUtils';
import { normalizeTranscriptSegment } from '../../../utils/transcriptTiming';
import { ContextMenuProvider } from '../../context-menu/ContextMenuProvider';
import { SegmentItem } from '../SegmentItem';
import { TranscriptUIContext, type TranscriptUIState } from '../TranscriptUIContext';

// Mock i18n
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

// Mock Icons
vi.mock('../../Icons', () => ({
  EditIcon: () => <span data-testid="edit-icon" />,
  TrashIcon: () => <span data-testid="trash-icon" />,
  MergeIcon: () => <span data-testid="merge-icon" />,
  CheckIcon: () => <span data-testid="check-icon" />,
  CloseIcon: () => <span data-testid="close-icon" />,
}));

// Mock SegmentTimestamp
vi.mock('../SegmentTimestamp', () => ({
  SegmentTimestamp: ({ start }: { start: number }) => <span>{start}</span>,
}));

describe('SegmentItem Highlighting', () => {
  let uiStore: any;

  const segment = normalizeTranscriptSegment({
    id: 'test-seg',
    start: 0,
    end: 5,
    text: 'Hello world test',
    isFinal: true,
    tokens: ['Hello', 'world', 'test'],
    timestamps: [0.0, 1.5, 3.0],
  });

  const defaultProps = {
    segment,
    index: 0,
    onSeek: vi.fn(),
    onEdit: vi.fn(),
    onSave: vi.fn(),
    onDelete: vi.fn(),
    onMergeWithNext: vi.fn(),
    onAnimationEnd: vi.fn(),
  };

  beforeEach(() => {
    useTranscriptStore.setState({ currentTime: 0 });
    uiStore = createStore<TranscriptUIState>(() => ({
      newSegmentIds: new Set(),
      activeSegmentId: 'test-seg', // Active segment
      editingSegmentId: null,
      totalSegments: 1,
      aligningSegmentIds: new Set(),
    }));
  });

  const renderComponent = () =>
    render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...defaultProps} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

  it('highlights the first token at start time', () => {
    // currentTime 0 (start)
    useTranscriptStore.setState({ currentTime: 0 });
    renderComponent();

    const token0 = screen.getByText('Hello');
    const token1 = screen.getByText('world');

    expect(token0.className).toContain('active-token');
    expect(token1.className).not.toContain('active-token');
  });

  it('highlights the second token when time advances', () => {
    useTranscriptStore.setState({ currentTime: 2.0 }); // 2.0 > 1.5 (world starts at 1.5)
    renderComponent();

    const token0 = screen.getByText('Hello');
    const token1 = screen.getByText('world');
    const token2 = screen.getByText('test');

    expect(token0.className).not.toContain('active-token');
    expect(token1.className).toContain('active-token');
    expect(token2.className).not.toContain('active-token');
  });

  it('highlights the last token when time is near end', () => {
    useTranscriptStore.setState({ currentTime: 4.0 }); // 4.0 > 3.0 (test starts at 3.0)
    renderComponent();

    const token1 = screen.getByText('world');
    const token2 = screen.getByText('test');

    expect(token1.className).not.toContain('active-token');
    expect(token2.className).toContain('active-token');
  });

  it('updates highlighting when store updates (re-render check)', async () => {
    useTranscriptStore.setState({ currentTime: 0 });
    renderComponent();

    expect(screen.getByText('Hello').className).toContain('active-token');

    // Update store
    act(() => {
      useTranscriptStore.setState({ currentTime: 2.0 });
    });

    // Re-render implicitly handled by store subscription?
    // Wait, testing-library render doesn't auto-update from external store unless component re-renders.
    // Zustand `useStore` triggers React re-render.

    expect(screen.getByText('world').className).toContain('active-token');
    expect(screen.getByText('Hello').className).not.toContain('active-token');
  });

  it('does not highlight tokens if segment is not active', () => {
    useTranscriptStore.setState({ currentTime: 0 });
    uiStore.setState({ activeSegmentId: 'other-seg' });
    renderComponent();

    const token0 = screen.getByText('Hello');
    expect(token0.className).not.toContain('active-token');
  });
});

describe('SegmentItem Translation Display', () => {
  let uiStore: any;

  const segmentWithTranslation = normalizeTranscriptSegment({
    id: 'trans-seg',
    start: 0,
    end: 5,
    text: 'Hello world',
    translation: '你好世界',
    isFinal: true,
    tokens: ['Hello', 'world'],
    timestamps: [0.0, 1.5],
  });

  const segmentWithoutTranslation = normalizeTranscriptSegment({
    id: 'no-trans-seg',
    start: 0,
    end: 5,
    text: 'Hello world',
    isFinal: true,
    tokens: ['Hello', 'world'],
    timestamps: [0.0, 1.5],
  });

  const createProps = (seg: any) => ({
    segment: seg,
    index: 0,
    onSeek: vi.fn(),
    onEdit: vi.fn(),
    onSave: vi.fn(),
    onDelete: vi.fn(),
    onMergeWithNext: vi.fn(),
    onAnimationEnd: vi.fn(),
  });

  beforeEach(() => {
    useTranscriptStore.setState({
      sourceHistoryId: null,
      llmStates: {},
    });
    uiStore = createStore<TranscriptUIState>(() => ({
      newSegmentIds: new Set(),
      activeSegmentId: null,
      editingSegmentId: null,
      totalSegments: 1,
      aligningSegmentIds: new Set(),
    }));
  });

  it('shows translation by default when translation exists (bilingual display)', () => {
    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...createProps(segmentWithTranslation)} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    const translationEl = container.querySelector('.segment-translation');
    expect(translationEl).not.toBeNull();
    expect(translationEl?.textContent).toBe('你好世界');
  });

  it('does not show translation element when segment has no translation', () => {
    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...createProps(segmentWithoutTranslation)} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    const translationEl = container.querySelector('.segment-translation');
    expect(translationEl).toBeNull();
  });

  it('hides translation when isTranslationVisible is set to false', () => {
    useTranscriptStore.getState().setIsTranslationVisible(false);

    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...createProps(segmentWithTranslation)} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    const translationEl = container.querySelector('.segment-translation');
    expect(translationEl).toBeNull();
  });

  it('shows translation by default even when sourceHistoryId is set without pre-existing llmState', () => {
    useTranscriptStore.setState({
      sourceHistoryId: 'history-item-123',
      llmStates: {},
    });

    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...createProps(segmentWithTranslation)} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    const translationEl = container.querySelector('.segment-translation');
    expect(translationEl).not.toBeNull();
    expect(translationEl?.textContent).toBe('你好世界');
  });

  it('keeps translation visible when original text is being edited', () => {
    uiStore.setState({ editingSegmentId: segmentWithTranslation.id });

    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...createProps(segmentWithTranslation)} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    const translationEl = container.querySelector('.segment-translation');
    expect(translationEl).not.toBeNull();
    expect(translationEl?.textContent).toBe('你好世界');
  });

  it('starts editing translation on double click', () => {
    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...createProps(segmentWithTranslation)} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    const translationEl = container.querySelector('.segment-translation');
    expect(translationEl).not.toBeNull();
    fireEvent.doubleClick(translationEl!);

    const inputEl = container.querySelector('.segment-translation-input');
    expect(inputEl).not.toBeNull();
    expect((inputEl as HTMLTextAreaElement).value).toBe('你好世界');
  });

  it('starts editing translation on edit button click', () => {
    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...createProps(segmentWithTranslation)} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    const editBtn = container.querySelector('.segment-translation-edit-btn');
    expect(editBtn).not.toBeNull();
    fireEvent.click(editBtn!);

    const inputEl = container.querySelector('.segment-translation-input');
    expect(inputEl).not.toBeNull();
  });

  it('saves translation on Enter key and calls onSaveTranslation', () => {
    const onSaveTranslation = vi.fn();
    const props = { ...createProps(segmentWithTranslation), onSaveTranslation };

    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...props} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    fireEvent.click(container.querySelector('.segment-translation-edit-btn')!);
    const inputEl = container.querySelector('.segment-translation-input') as HTMLTextAreaElement;

    fireEvent.change(inputEl, { target: { value: '更新后的译文' } });
    fireEvent.keyDown(inputEl, { key: 'Enter' });

    expect(onSaveTranslation).toHaveBeenCalledWith('trans-seg', '更新后的译文');
    expect(container.querySelector('.segment-translation-editor')).toBeNull();
  });

  it('saves translation on Save button click', () => {
    const onSaveTranslation = vi.fn();
    const props = { ...createProps(segmentWithTranslation), onSaveTranslation };

    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...props} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    fireEvent.click(container.querySelector('.segment-translation-edit-btn')!);
    const inputEl = container.querySelector('.segment-translation-input') as HTMLTextAreaElement;

    fireEvent.change(inputEl, { target: { value: '按钮保存的译文' } });
    fireEvent.click(container.querySelector('.segment-translation-save-btn')!);

    expect(onSaveTranslation).toHaveBeenCalledWith('trans-seg', '按钮保存的译文');
    expect(container.querySelector('.segment-translation-editor')).toBeNull();
  });

  it('cancels translation editing on Escape without saving', () => {
    const onSaveTranslation = vi.fn();
    const props = { ...createProps(segmentWithTranslation), onSaveTranslation };

    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...props} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    fireEvent.click(container.querySelector('.segment-translation-edit-btn')!);
    const inputEl = container.querySelector('.segment-translation-input') as HTMLTextAreaElement;

    fireEvent.change(inputEl, { target: { value: '未保存的译文' } });
    fireEvent.keyDown(inputEl, { key: 'Escape' });

    expect(onSaveTranslation).not.toHaveBeenCalled();
    expect(container.querySelector('.segment-translation-editor')).toBeNull();
    expect(container.querySelector('.segment-translation')?.textContent).toBe('你好世界');
  });

  it('cancels translation editing on Cancel button click without saving', () => {
    const onSaveTranslation = vi.fn();
    const props = { ...createProps(segmentWithTranslation), onSaveTranslation };

    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...props} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    fireEvent.click(container.querySelector('.segment-translation-edit-btn')!);
    const inputEl = container.querySelector('.segment-translation-input') as HTMLTextAreaElement;

    fireEvent.change(inputEl, { target: { value: '未保存的译文' } });
    fireEvent.click(container.querySelector('.segment-translation-cancel-btn')!);

    expect(onSaveTranslation).not.toHaveBeenCalled();
    expect(container.querySelector('.segment-translation-editor')).toBeNull();
  });

  it('saves translation on blur', () => {
    const onSaveTranslation = vi.fn();
    const props = { ...createProps(segmentWithTranslation), onSaveTranslation };

    const { container } = render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem {...props} />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    fireEvent.click(container.querySelector('.segment-translation-edit-btn')!);
    const inputEl = container.querySelector('.segment-translation-input') as HTMLTextAreaElement;

    fireEvent.change(inputEl, { target: { value: '失焦保存的译文' } });
    fireEvent.blur(inputEl);

    expect(onSaveTranslation).toHaveBeenCalledWith('trans-seg', '失焦保存的译文');
    expect(container.querySelector('.segment-translation-editor')).toBeNull();
  });
});
