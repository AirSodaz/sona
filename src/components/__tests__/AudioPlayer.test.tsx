import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
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
        // Reset store state
        useTranscriptStore.setState({
            audioUrl: 'test-audio.mp3',
            isPlaying: false,
            currentTime: 0,
        });
    });

    it('renders correctly', () => {
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
});
