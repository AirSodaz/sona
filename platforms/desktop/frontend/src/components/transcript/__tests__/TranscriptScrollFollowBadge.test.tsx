import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import TranscriptScrollFollowBadge from '../TranscriptScrollFollowBadge';

// Mock i18n
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number; defaultValue?: string }) => {
      if (options?.defaultValue) {
        return options.defaultValue.replace('{count}', String(options.count ?? ''));
      }
      return key;
    },
  }),
}));

describe('TranscriptScrollFollowBadge', () => {
  it('renders word count only when follow is not detached', () => {
    const onResume = vi.fn();
    render(<TranscriptScrollFollowBadge detachMode={null} onResume={onResume} wordCount={120} />);

    expect(screen.getByText('120 words')).toBeDefined();
    expect(screen.queryByTestId('scroll-follow-resume-btn')).toBeNull();
  });

  it('renders streaming follow button and calls onResume when clicked', () => {
    const onResume = vi.fn();
    render(
      <TranscriptScrollFollowBadge detachMode="streaming" onResume={onResume} wordCount={350} />
    );

    expect(screen.getByText('350 words')).toBeDefined();
    const btn = screen.getByTestId('scroll-follow-resume-btn');
    expect(btn).toBeDefined();
    expect(btn.getAttribute('data-tooltip')).toBe('Scroll to bottom and resume follow');

    fireEvent.click(btn);
    expect(onResume).toHaveBeenCalledTimes(1);
  });

  it('renders playback locate button and calls onResume when clicked', () => {
    const onResume = vi.fn();
    render(
      <TranscriptScrollFollowBadge detachMode="playback" onResume={onResume} wordCount={500} />
    );

    const btn = screen.getByTestId('scroll-follow-resume-btn');
    expect(btn).toBeDefined();
    expect(btn.getAttribute('data-tooltip')).toBe('Locate playing segment');

    fireEvent.click(btn);
    expect(onResume).toHaveBeenCalledTimes(1);
  });

  it('keeps button mounted with disabled pointer-events during collapse animation, then unmounts', () => {
    vi.useFakeTimers();
    try {
      const onResume = vi.fn();
      const { rerender } = render(
        <TranscriptScrollFollowBadge detachMode="streaming" onResume={onResume} wordCount={200} />
      );

      const btn = screen.getByTestId('scroll-follow-resume-btn');
      expect(btn).toBeDefined();

      // Transition to null
      act(() => {
        rerender(
          <TranscriptScrollFollowBadge detachMode={null} onResume={onResume} wordCount={200} />
        );
      });

      // Button is still mounted for smooth exit animation, but inert
      const exitingBtn = screen.getByTestId('scroll-follow-resume-btn');
      expect(exitingBtn).toBeDefined();
      expect(exitingBtn.getAttribute('aria-hidden')).toBe('true');
      expect(exitingBtn.getAttribute('tabindex')).toBe('-1');

      // Fast-forward past the 300ms transition timer
      act(() => {
        vi.advanceTimersByTime(350);
      });

      // Button should now be unmounted
      expect(screen.queryByTestId('scroll-follow-resume-btn')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});
