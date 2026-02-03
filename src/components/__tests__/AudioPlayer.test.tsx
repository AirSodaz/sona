import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { AudioPlayer } from '../AudioPlayer';
import { useTranscriptStore } from '../../stores/transcriptStore';

// Mock dependencies
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

describe('AudioPlayer', () => {
    beforeEach(() => {
        // Mock URL.createObjectURL/revokeObjectURL
        global.URL.createObjectURL = vi.fn(() => 'blob:test');
        global.URL.revokeObjectURL = vi.fn();

        // Reset store
        useTranscriptStore.setState({
            audioUrl: 'blob:test',
            currentTime: 0,
            isPlaying: false,
            segments: []
        });
    });

    afterEach(() => {
        vi.clearAllMocks();
    });

    it('renders time display and slider', () => {
        render(<AudioPlayer />);
        const times = screen.getAllByText('00:00.0');
        expect(times.length).toBeGreaterThan(0);
        // Seek slider
        expect(screen.getByLabelText('player.seek')).toBeDefined();
    });

    it('updates time display when store updates', async () => {
        const { container } = render(<AudioPlayer />);

        // Initial state: two 00:00.0 (current and total)
        // The first .audio-time is the current time
        const timeDisplay = container.querySelector('.audio-time');
        expect(timeDisplay?.textContent).toBe('00:00.0');

        // Update store
        act(() => {
            useTranscriptStore.setState({ currentTime: 61.5 });
        });

        // Should see 01:01.5
        expect(timeDisplay?.textContent).toBe('01:01.5');
    });

    it('updates slider value when store updates', async () => {
        const { container } = render(<AudioPlayer />);

        // Set duration so slider can move
        const audio = container.querySelector('audio');
        if (audio) {
            Object.defineProperty(audio, 'duration', { value: 100, writable: true });
            fireEvent.loadedMetadata(audio);
        }

        const slider = screen.getByLabelText('player.seek') as HTMLInputElement;

        act(() => {
            useTranscriptStore.setState({ currentTime: 10.5 });
        });

        expect(slider.value).toBe('10.5');
    });
});
