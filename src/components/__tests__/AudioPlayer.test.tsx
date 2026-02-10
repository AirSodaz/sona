import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { act } from 'react';
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

        // Mock HTMLMediaElement methods
        HTMLMediaElement.prototype.play = vi.fn().mockResolvedValue(undefined);
        HTMLMediaElement.prototype.pause = vi.fn();
        HTMLMediaElement.prototype.load = vi.fn();
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

    it('updates slider accessibility attributes when store updates', async () => {
        render(<AudioPlayer />);
        const slider = screen.getByLabelText('player.seek') as HTMLInputElement;

        act(() => {
            useTranscriptStore.setState({ currentTime: 61.5 });
        });

        expect(slider.getAttribute('aria-valuenow')).toBe('61.5');
        expect(slider.getAttribute('aria-valuetext')).toBe('01:01.5');
    });

    it('volume slider has accessibility attributes', () => {
        render(<AudioPlayer />);
        const volumeSlider = screen.getByLabelText('player.volume') as HTMLInputElement;

        expect(volumeSlider.getAttribute('aria-valuemin')).toBe('0');
        expect(volumeSlider.getAttribute('aria-valuemax')).toBe('1');
        expect(volumeSlider.getAttribute('aria-valuenow')).toBe('1');
        expect(volumeSlider.getAttribute('aria-valuetext')).toBe('100%');
    });

    it('shows keyboard shortcuts in tooltips', async () => {
        render(<AudioPlayer />);

        const playButton = screen.getByLabelText('player.play');
        // Simulate hover
        fireEvent.mouseEnter(playButton.parentElement as Element);

        // Wait for tooltip to appear (it has a timeout)
        const tooltip = await screen.findByText('Space');
        expect(tooltip.tagName).toBe('KBD');
        expect(tooltip.parentElement?.textContent).toContain('player.play');

        const muteButton = screen.getByLabelText('player.mute');
        fireEvent.mouseEnter(muteButton.parentElement as Element);

        const muteTooltip = await screen.findByText('M');
        expect(muteTooltip.tagName).toBe('KBD');
        expect(muteTooltip.parentElement?.textContent).toContain('player.mute');
    });

    describe('Keyboard Shortcuts', () => {
        it('toggles play/pause with Space', () => {
            const { container } = render(<AudioPlayer />);
            const audio = container.querySelector('audio');

            // Mock audio play/pause methods
            if (audio) {
                Object.defineProperty(audio, 'paused', { value: true, writable: true });
                audio.play = vi.fn().mockImplementation(async () => {
                    Object.defineProperty(audio, 'paused', { value: false, writable: true });
                });
                audio.pause = vi.fn().mockImplementation(() => {
                    Object.defineProperty(audio, 'paused', { value: true, writable: true });
                });
            }

            // Initially paused
            expect(useTranscriptStore.getState().isPlaying).toBe(false);

            // Press Space -> Play
            fireEvent.keyDown(window, { key: ' ' });
            expect(useTranscriptStore.getState().isPlaying).toBe(true);
            expect(audio?.play).toHaveBeenCalled();

            // Press Space -> Pause
            fireEvent.keyDown(window, { key: ' ' });
            expect(useTranscriptStore.getState().isPlaying).toBe(false);
            expect(audio?.pause).toHaveBeenCalled();
        });

        it('seeks with ArrowLeft and ArrowRight', () => {
            const { container } = render(<AudioPlayer />);
            const audio = container.querySelector('audio');

            // Mock audio duration
            if (audio) {
                Object.defineProperty(audio, 'duration', { value: 100, writable: true });
                // We need to set currentTime manually as JSDOM audio doesn't update it automatically
                Object.defineProperty(audio, 'currentTime', { value: 50, writable: true });
            }

            // Move to 50s
            act(() => {
                useTranscriptStore.setState({ currentTime: 50 });
            });

            // ArrowLeft (-5s)
            fireEvent.keyDown(window, { key: 'ArrowLeft' });
            expect(useTranscriptStore.getState().currentTime).toBe(45);

            // ArrowRight (+5s)
            // Note: In real app, seekTo updates state. Here we mock it via store since our seekTo implementation uses store actions.
            // Wait, our seekTo implementation updates store AND audio.currentTime. 
            // The test might be tricky because we rely on the component's internal `audioRef.current.currentTime` to read start time.
            // Let's verify if seekTo calls `setCurrentTime`.

            // Reset to 50
            act(() => {
                const audio = container.querySelector('audio');
                if (audio) audio.currentTime = 50;
                useTranscriptStore.setState({ currentTime: 50 });
            });

            fireEvent.keyDown(window, { key: 'ArrowRight' });
            expect(useTranscriptStore.getState().currentTime).toBe(55);
        });

        it('adjusts volume with ArrowUp and ArrowDown', () => {
            const { container } = render(<AudioPlayer />);
            const audio = container.querySelector('audio');

            // Initially volume is 1

            // ArrowDown (-0.1) -> 0.9
            fireEvent.keyDown(window, { key: 'ArrowDown' });
            expect(audio?.volume).toBeCloseTo(0.9);

            // ArrowUp (+0.1) -> 1.0
            fireEvent.keyDown(window, { key: 'ArrowUp' });
            expect(audio?.volume).toBe(1);
        });

        it('toggles mute with m', () => {
            const { container } = render(<AudioPlayer />);
            const audio = container.querySelector('audio');

            // Initially unmuted
            expect(audio?.muted).toBe(false);

            // Press m -> Mute
            fireEvent.keyDown(window, { key: 'm' });
            expect(audio?.muted).toBe(true);

            // Press m -> Unmute
            fireEvent.keyDown(window, { key: 'm' });
            expect(audio?.muted).toBe(false);
        });

        it('ignores shortcuts when input is focused', () => {
            const { container } = render(
                <div>
                    <AudioPlayer />
                    <input data-testid="test-input" />
                </div>
            );

            const audio = container.querySelector('audio');
            if (audio) {
                audio.play = vi.fn().mockResolvedValue(undefined);
            }

            const input = screen.getByTestId('test-input');
            input.focus();

            // Press Space while focused on input
            fireEvent.keyDown(input, { key: ' ', bubbles: true });

            // Should NOT play
            expect(useTranscriptStore.getState().isPlaying).toBe(false);
            expect(audio?.play).not.toHaveBeenCalled();
        });
    });
});
