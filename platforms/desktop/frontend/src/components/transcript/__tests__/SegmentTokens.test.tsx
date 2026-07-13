
import { beforeEach, describe, it, expect, vi } from 'vitest';
import type { ReactElement } from 'react';
import {
  render as testingLibraryRender,
  screen,
  fireEvent,
  waitFor,
} from '@testing-library/react';
import { SegmentTokens } from '../SegmentTokens';
import { TranscriptSegment } from '../../../types/transcript';
import { Match } from '../../../stores/searchStore';
import { ContextMenuProvider } from '../../context-menu/ContextMenuProvider';

const loggerErrorMock = vi.hoisted(() => vi.fn());

// Mock dependencies
vi.mock('../../../utils/exportFormats', () => ({
  formatDisplayTime: (time: number) => `Time: ${time}`
}));

vi.mock('../../../utils/logger', () => ({
  logger: {
    error: loggerErrorMock,
  },
}));

const render = (ui: ReactElement) => testingLibraryRender(
  <ContextMenuProvider>{ui}</ContextMenuProvider>,
);

describe('SegmentTokens', () => {
  const mockSegment: TranscriptSegment = {
    id: 'seg-1',
    text: 'Hello world',
    start: 0,
    end: 2,
    isFinal: true,
    timing: {
      level: 'token',
      source: 'model',
      units: [
        { text: 'Hello', start: 0, end: 0.5 },
        { text: ' ', start: 0.5, end: 1.0 },
        { text: 'world', start: 1.0, end: 2.0 },
      ],
    },
  };

  const mockOnSeek = vi.fn();
  const mockOnMatchClick = vi.fn();

  const writeTextMock = vi.fn().mockResolvedValue(undefined);

  const renderWithContextMenu = (onSeek = vi.fn()) => render(
    <SegmentTokens
      segment={mockSegment}
      isActive={false}
      onSeek={onSeek}
    />,
  );

  const getSegmentRoot = (container: HTMLElement) => {
    const root = container.querySelector<HTMLElement>('p.segment-text');
    expect(root).not.toBeNull();
    return root as HTMLElement;
  };

  beforeEach(() => {
    vi.clearAllMocks();
    writeTextMock.mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: writeTextMock },
    });
    window.getSelection()?.removeAllRanges();
  });

  it('renders segment text correctly', () => {
    render(
      <SegmentTokens
        segment={mockSegment}
        isActive={false}
        onSeek={mockOnSeek}
      />
    );

    expect(screen.getByText('Hello')).not.toBeNull();
    expect(screen.getByText('world')).not.toBeNull();
  });

  it('renders edited segment text when timing text is stale', () => {
    render(
      <SegmentTokens
        segment={{
          ...mockSegment,
          text: 'Edited text',
        }}
        isActive={false}
        onSeek={mockOnSeek}
      />
    );

    expect(screen.getByText('Edited text')).not.toBeNull();
    expect(screen.queryByText('Hello')).toBeNull();
    expect(screen.queryByText('world')).toBeNull();
  });

  it('renders edited punctuation when timing text differs only by punctuation', () => {
    render(
      <SegmentTokens
        segment={{
          ...mockSegment,
          text: 'Hello, world',
        }}
        isActive={false}
        onSeek={mockOnSeek}
      />
    );

    expect(screen.getByText('Hello, world')).not.toBeNull();
  });

  it('applies "partial" class when segment is not final', () => {
    const partialSegment = { ...mockSegment, isFinal: false };
    const { container } = render(
      <SegmentTokens
        segment={partialSegment}
        isActive={false}
        onSeek={mockOnSeek}
      />
    );

    const paragraph = container.querySelector('p.segment-text');
    expect(paragraph).not.toBeNull();
    expect(paragraph?.classList.contains('partial')).toBe(true);
  });

  it('handles token click (seek)', () => {
    render(
      <SegmentTokens
        segment={mockSegment}
        isActive={false}
        onSeek={mockOnSeek}
      />
    );

    const token = screen.getByText('Hello');
    fireEvent.click(token);

    expect(mockOnSeek).toHaveBeenCalledWith(0);
  });

  it('uses the standard custom tooltip for timed tokens', () => {
    render(
      <SegmentTokens
        segment={mockSegment}
        isActive={false}
        onSeek={mockOnSeek}
      />
    );

    const token = screen.getByText('Hello');
    expect(token.getAttribute('title')).toBeNull();
    expect(token.getAttribute('data-tooltip')).toBe('Time: 0');
    expect(token.getAttribute('data-tooltip-pos')).toBe('top');
  });

  it('highlights search matches correctly', () => {
    const matches: Match[] = [
      { startIndex: 0, length: 5, globalIndex: 0, segmentId: 'seg-1', text: 'Hello' } // Matches "Hello"
    ];

    render(
      <SegmentTokens
        segment={mockSegment}
        isActive={false}
        onSeek={mockOnSeek}
        matches={matches}
        activeMatch={null}
        onMatchClick={mockOnMatchClick}
      />
    );

    const token = screen.getByText('Hello');
    expect(token.classList.contains('search-match')).toBe(true);
    expect(token.classList.contains('search-match-active')).toBe(false);
  });

  it('highlights active match correctly', () => {
    const activeMatch: Match = { startIndex: 0, length: 5, globalIndex: 0, segmentId: 'seg-1', text: 'Hello' }; // Matches "Hello"
    const matches: Match[] = [activeMatch];

    render(
      <SegmentTokens
        segment={mockSegment}
        isActive={false}
        onSeek={mockOnSeek}
        matches={matches}
        activeMatch={activeMatch}
        onMatchClick={mockOnMatchClick}
      />
    );

    const token = screen.getByText('Hello');
    expect(token.classList.contains('search-match-active')).toBe(true);
  });

  it('handles match click', () => {
    const matches: Match[] = [
      { startIndex: 0, length: 5, globalIndex: 1, segmentId: 'seg-1', text: 'Hello' } // Matches "Hello"
    ];

    render(
      <SegmentTokens
        segment={mockSegment}
        isActive={false}
        onSeek={mockOnSeek}
        matches={matches}
        activeMatch={null}
        onMatchClick={mockOnMatchClick}
      />
    );

    const token = screen.getByText('Hello');
    fireEvent.click(token);

    expect(mockOnMatchClick).toHaveBeenCalledWith(1);
    expect(mockOnSeek).toHaveBeenCalledWith(0);
  });

  it('copies a non-empty selection contained by the current segment', async () => {
    const { container } = renderWithContextMenu();
    const root = getSegmentRoot(container);
    const helloText = screen.getByText('Hello').firstChild;
    expect(helloText).not.toBeNull();

    const range = document.createRange();
    range.setStart(helloText as Text, 1);
    range.setEnd(helloText as Text, 5);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    fireEvent.contextMenu(root, { clientX: 80, clientY: 120 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Copy' }));

    await waitFor(() => {
      expect(writeTextMock).toHaveBeenCalledWith('ello');
    });
  });

  it('copies the rendered segment text when there is no contained selection', async () => {
    const { container } = renderWithContextMenu();
    const root = getSegmentRoot(container);

    window.getSelection()?.removeAllRanges();
    fireEvent.contextMenu(root, { clientX: 80, clientY: 120 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Copy' }));

    await waitFor(() => {
      expect(writeTextMock).toHaveBeenCalledWith('Hello world');
    });
  });

  it('falls back to the segment text when the selection crosses its boundary', async () => {
    const { container } = renderWithContextMenu();
    const root = getSegmentRoot(container);
    const outside = document.createElement('span');
    outside.textContent = 'outside';
    root.after(outside);
    const helloText = screen.getByText('Hello').firstChild as Text;
    const outsideText = outside.firstChild as Text;
    const range = document.createRange();
    range.setStart(helloText, 0);
    range.setEnd(outsideText, outsideText.length);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    fireEvent.contextMenu(root, { clientX: 80, clientY: 120 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Copy' }));

    await waitFor(() => expect(writeTextMock).toHaveBeenCalledWith('Hello world'));
  });

  it('selects only the rendered content of the current segment', () => {
    const { container } = renderWithContextMenu();
    const root = getSegmentRoot(container);

    fireEvent.contextMenu(root, { clientX: 80, clientY: 120 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Select All' }));

    const selection = window.getSelection();
    expect(selection?.toString()).toBe('Hello world');
    expect(root.contains(selection?.getRangeAt(0).startContainer ?? null)).toBe(true);
    expect(root.contains(selection?.getRangeAt(0).endContainer ?? null)).toBe(true);
  });

  it.each([
    { key: 'F10', shiftKey: true },
    { key: 'ContextMenu', shiftKey: false },
  ])('opens from $key and restores focus on Escape', ({ key, shiftKey }) => {
    const { container } = renderWithContextMenu();
    const root = getSegmentRoot(container);
    root.focus();

    fireEvent.keyDown(root, { key, shiftKey });

    const menu = screen.getByRole('menu', { name: 'Text editing actions' });
    expect(menu).not.toBeNull();
    fireEvent.keyDown(menu, { key: 'Escape' });
    expect(document.activeElement).toBe(root);
  });

  it('does not seek when opening or using the readonly segment menu', () => {
    const onSeek = vi.fn();
    renderWithContextMenu(onSeek);

    fireEvent.contextMenu(screen.getByText('Hello'), { clientX: 80, clientY: 120 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Select All' }));

    expect(onSeek).not.toHaveBeenCalled();
  });

  it('disables copy when the Clipboard API is unavailable', () => {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: undefined,
    });
    const { container } = renderWithContextMenu();

    fireEvent.contextMenu(getSegmentRoot(container), { clientX: 80, clientY: 120 });

    expect((screen.getByRole('menuitem', { name: 'Copy' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('logs clipboard failures without surfacing them', async () => {
    const copyError = new Error('clipboard denied');
    writeTextMock.mockRejectedValueOnce(copyError);
    const { container } = renderWithContextMenu();

    fireEvent.contextMenu(getSegmentRoot(container), { clientX: 80, clientY: 120 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Copy' }));

    await waitFor(() => {
      expect(loggerErrorMock).toHaveBeenCalledWith(
        '[ReadonlySegmentContextMenu] Failed to copy text:',
        copyError,
      );
    });
  });

  it('closes the readonly menu when its virtualized segment unmounts', () => {
    const result = renderWithContextMenu();
    const root = getSegmentRoot(result.container);
    fireEvent.contextMenu(root, { clientX: 80, clientY: 120 });
    expect(screen.getByRole('menu')).not.toBeNull();

    result.rerender(
      <ContextMenuProvider>
        <div>Segment removed</div>
      </ContextMenuProvider>,
    );

    expect(screen.queryByRole('menu')).toBeNull();
  });
});
