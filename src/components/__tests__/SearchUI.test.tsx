
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SearchUI } from '../SearchUI';

// Mock stores
const mockUseSearchStore = vi.fn();
const mockUseTranscriptStore = vi.fn();

vi.mock('../../stores/searchStore', () => ({
  useSearchStore: () => mockUseSearchStore()
}));

vi.mock('../../stores/transcriptStore', () => ({
  useTranscriptStore: (selector: any) => mockUseTranscriptStore(selector)
}));

// Mock icons
vi.mock('../Icons', () => ({
  ChevronUpIcon: () => <span data-testid="chevron-up">Up</span>,
  ChevronDownIcon: () => <span data-testid="chevron-down">Down</span>,
  CloseIcon: () => <span data-testid="close">Close</span>
}));

// Mock translation
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue || key
  })
}));

describe('SearchUI', () => {
  const defaultSearchState = {
    isOpen: true,
    query: '',
    matches: [],
    currentMatchIndex: 0,
    close: vi.fn(),
    setQuery: vi.fn(),
    nextMatch: vi.fn(),
    prevMatch: vi.fn(),
    performSearch: vi.fn()
  };

  beforeEach(() => {
    vi.clearAllMocks();
    mockUseSearchStore.mockReturnValue(defaultSearchState);
    mockUseTranscriptStore.mockImplementation((selector: any) => {
      // If selector is a function, call it with mock state
      if (typeof selector === 'function') {
        return selector({ segments: [] });
      }
      return [];
    });
  });

  it('renders nothing when not open', () => {
    mockUseSearchStore.mockReturnValue({ ...defaultSearchState, isOpen: false });
    const { container } = render(<SearchUI />);
    expect(container.firstChild).toBeNull();
  });

  it('renders search input when open', () => {
    render(<SearchUI />);
    expect(screen.getByRole('search')).not.toBeNull();
    expect(screen.getByPlaceholderText('Find in transcript...')).not.toBeNull();
  });

  it('updates query on input change', () => {
    render(<SearchUI />);
    const input = screen.getByPlaceholderText('Find in transcript...');
    fireEvent.change(input, { target: { value: 'hello' } });
    expect(defaultSearchState.setQuery).toHaveBeenCalledWith('hello');
  });

  it('displays match count correctly (0/0)', () => {
    mockUseSearchStore.mockReturnValue({
      ...defaultSearchState,
      query: 'foo',
      matches: [],
      currentMatchIndex: 0
    });
    render(<SearchUI />);
    // "0/0" is rendered when query exists but matches is empty
    expect(screen.getByText('0/0')).not.toBeNull();
  });

  it('displays match count correctly with matches (1/5)', () => {
    mockUseSearchStore.mockReturnValue({
      ...defaultSearchState,
      query: 'foo',
      matches: [1, 2, 3, 4, 5], // Mock matches array
      currentMatchIndex: 0
    });
    render(<SearchUI />);
    expect(screen.getByText('1/5')).not.toBeNull();
  });

  it('displays empty string when no query', () => {
      mockUseSearchStore.mockReturnValue({
        ...defaultSearchState,
        query: '',
        matches: [],
        currentMatchIndex: 0
      });
      render(<SearchUI />);
      // Should not show 0/0. Text should be empty.
      // Getting empty text is tricky with getByText.
      // We can query by class name.
      const { container } = render(<SearchUI />);
      const count = container.querySelector('.search-count');
      expect(count).not.toBeNull();
      expect(count?.textContent).toBe('');
  });

  it('calls nextMatch on Enter', () => {
    render(<SearchUI />);
    const input = screen.getByPlaceholderText('Find in transcript...');
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(defaultSearchState.nextMatch).toHaveBeenCalled();
  });

  it('calls prevMatch on Shift+Enter', () => {
    render(<SearchUI />);
    const input = screen.getByPlaceholderText('Find in transcript...');
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: true });
    expect(defaultSearchState.prevMatch).toHaveBeenCalled();
  });

  it('calls close on Escape', () => {
    render(<SearchUI />);
    const input = screen.getByPlaceholderText('Find in transcript...');
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(defaultSearchState.close).toHaveBeenCalled();
  });
});
