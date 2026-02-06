import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { RecordingTimer } from '../RecordingTimer';

// Mock translation
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => {
            if (key === 'live.timer_label') return 'Recording duration';
            return key;
        },
    }),
}));

describe('RecordingTimer', () => {
    beforeEach(() => {
        vi.useFakeTimers();
    });

    afterEach(() => {
        vi.useRealTimers();
        vi.clearAllMocks();
    });

    it('should render hidden when not recording', () => {
        render(<RecordingTimer isRecording={false} isPaused={false} />);

        const timerText = screen.getByText('00:00');
        // Check style manually since jest-dom matchers are not available
        expect(timerText.style.visibility).toBe('hidden');
    });

    it('should render visible with accessibility attributes when recording', () => {
        render(<RecordingTimer isRecording={true} isPaused={false} />);

        const timer = screen.getByRole('timer');
        expect(timer).toBeTruthy();
        expect(timer.style.visibility).toBe('visible');
        expect(timer.getAttribute('aria-label')).toBe('Recording duration');
        expect(timer.getAttribute('aria-live')).toBe('off');
        expect(timer.textContent).toBe('00:00');
    });

    it('should update time every second', () => {
        render(<RecordingTimer isRecording={true} isPaused={false} />);

        const timer = screen.getByRole('timer');
        expect(timer.textContent).toBe('00:00');

        act(() => {
            vi.advanceTimersByTime(1000);
        });
        expect(timer.textContent).toBe('00:01');

        act(() => {
            vi.advanceTimersByTime(2000);
        });
        expect(timer.textContent).toBe('00:03');
    });

    it('should not update time when paused', () => {
        const { rerender } = render(<RecordingTimer isRecording={true} isPaused={false} />);

        const timer = screen.getByRole('timer');

        act(() => {
            vi.advanceTimersByTime(2000);
        });
        expect(timer.textContent).toBe('00:02');

        // Pause
        rerender(<RecordingTimer isRecording={true} isPaused={true} />);

        act(() => {
            vi.advanceTimersByTime(3000);
        });
        // Should stay at 00:02
        expect(timer.textContent).toBe('00:02');

        // Resume
        rerender(<RecordingTimer isRecording={true} isPaused={false} />);

        act(() => {
            vi.advanceTimersByTime(1000);
        });
        expect(timer.textContent).toBe('00:03');
    });

    it('should reset time when recording stops', () => {
        const { rerender } = render(<RecordingTimer isRecording={true} isPaused={false} />);

        const timer = screen.getByRole('timer');

        act(() => {
            vi.advanceTimersByTime(5000);
        });
        expect(timer.textContent).toBe('00:05');

        // Stop
        rerender(<RecordingTimer isRecording={false} isPaused={false} />);

        expect(timer.style.visibility).toBe('hidden');

        // Restart
        rerender(<RecordingTimer isRecording={true} isPaused={false} />);
        expect(timer.textContent).toBe('00:00');
    });
});
