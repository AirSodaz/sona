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

vi.mock('../../stores/dialogStore', () => ({
    useDialogStore: () => ({
        alert: vi.fn(),
    }),
}));

describe('AudioPlayer', () => {
    beforeEach(() => {
        // Mock URL.createObjectURL/revokeObjectURL
        global.URL.createObjectURL = vi.fn(() => 'blob:test');
        global.URL.revokeObjectURL = vi.fn();

        // Reset store state
        useTranscriptStore.setState({
            audioUrl: 'test-audio.mp3',
            isPlaying: false,
            currentTime: 0,
            segments: []
        });
    });

    afterEach(() => {
        vi.clearAllMocks();
    });

    it('renders mute and volume controls', () => {
        render(<AudioPlayer />);
        expect(screen.getByLabelText('player.play')).toBeDefined();
        expect(screen.getByLabelText('player.mute')).toBeDefined();
        expect(screen.getByLabelText('player.volume')).toBeDefined();
    });

    it('toggles mute state', () => {
        render(<AudioPlayer />);
        const muteButton = screen.getByLabelText('player.mute');

        // Click to mute
        fireEvent.click(muteButton);
        expect(screen.getByLabelText('player.unmute')).toBeDefined();

        // Click to unmute
        fireEvent.click(muteButton);
        expect(screen.getByLabelText('player.mute')).toBeDefined();
    });

    it('renders time display and slider', () => {
        render(<AudioPlayer />);
        const times = screen.getAllByText('00:00.0');
        expect(times.length).toBeGreaterThan(0);
        expect(screen.getByLabelText('player.seek')).toBeDefined();
    });

    it('updates time display when store updates', async () => {
        const { container } = render(<AudioPlayer />);

        const timeDisplay = container.querySelector('.audio-time');
        expect(timeDisplay?.textContent).toBe('00:00.0');

        act(() => {
            useTranscriptStore.setState({ currentTime: 61.5 });
        });

        expect(timeDisplay?.textContent).toBe('01:01.5');
    });

    it('updates slider value when store updates', async () => {
        const { container } = render(<AudioPlayer />);

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
