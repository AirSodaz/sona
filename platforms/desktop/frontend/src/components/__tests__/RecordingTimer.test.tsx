import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
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
    afterEach(() => {
        vi.clearAllMocks();
    });

    it('renders hidden when not recording', () => {
        render(<RecordingTimer elapsedMs={0} isRecording={false} />);

        const timer = screen.getByRole('timer', { hidden: true });
        expect(timer.style.visibility).toBe('hidden');
        expect(timer.textContent).toBe('00:00');
    });

    it('renders visible with accessibility attributes when recording', () => {
        render(<RecordingTimer elapsedMs={0} isRecording={true} />);

        const timer = screen.getByRole('timer');
        expect(timer.style.visibility).toBe('visible');
        expect(timer.getAttribute('aria-label')).toBe('Recording duration');
        expect(timer.getAttribute('aria-live')).toBe('off');
        expect(timer.textContent).toBe('00:00');
    });

    it('formats elapsed milliseconds into minutes and seconds', () => {
        const { rerender } = render(<RecordingTimer elapsedMs={1000} isRecording={true} />);

        const timer = screen.getByRole('timer');
        expect(timer.textContent).toBe('00:01');

        rerender(<RecordingTimer elapsedMs={61000} isRecording={true} />);
        expect(timer.textContent).toBe('01:01');
    });

    it('resets to zero when a new recording session starts', () => {
        const { rerender } = render(<RecordingTimer elapsedMs={5000} isRecording={true} />);

        const timer = screen.getByRole('timer');
        expect(timer.textContent).toBe('00:05');

        rerender(<RecordingTimer elapsedMs={0} isRecording={false} />);
        expect(timer.style.visibility).toBe('hidden');

        rerender(<RecordingTimer elapsedMs={0} isRecording={true} />);
        expect(timer.textContent).toBe('00:00');
    });
});
