
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SegmentTokens } from '../SegmentTokens';
import { TranscriptSegment } from '../../../types/transcript';
import { Match } from '../../../stores/searchStore';

// Mock dependencies
vi.mock('../../../utils/exportFormats', () => ({
  formatDisplayTime: (time: number) => `Time: ${time}`
}));

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
});
