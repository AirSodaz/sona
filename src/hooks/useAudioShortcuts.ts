import { useEffect, RefObject } from 'react';

interface AudioShortcutsProps {
    audioRef: RefObject<HTMLAudioElement | null>;
    seekTo: (time: number) => void;
    setIsPlaying: (isPlaying: boolean) => void;
    volume: number;
    setVolume: (volume: number) => void;
    toggleMute: () => void;
}

/**
 * Hook to handle keyboard shortcuts for audio playback.
 *
 * Supported keys:
 * - Space/K: Play/Pause
 * - ArrowLeft/Right: Seek -/+ 5s
 * - ArrowUp/Down: Volume +/- 10%
 * - M: Mute toggle
 *
 * @param props - Dependencies required for shortcut handling.
 */
export function useAudioShortcuts({
    audioRef,
    seekTo,
    setIsPlaying,
    volume,
    setVolume,
    toggleMute
}: AudioShortcutsProps) {
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            const audio = audioRef.current;
            if (!audio) return;

            // Ignore if user is typing in an input or textarea
            const target = e.target as HTMLElement;
            if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
                return;
            }

            switch (e.key) {
                case ' ':
                case 'k': // YouTube style pause
                    e.preventDefault();
                    if (audio.paused) {
                        audio.play().catch(console.error);
                        setIsPlaying(true);
                    } else {
                        audio.pause();
                        setIsPlaying(false);
                    }
                    break;
                case 'ArrowLeft':
                    e.preventDefault();
                    seekTo(Math.max(0, audio.currentTime - 5));
                    break;
                case 'ArrowRight':
                    e.preventDefault();
                    seekTo(Math.min(audio.duration || 0, audio.currentTime + 5));
                    break;
                case 'ArrowUp':
                    e.preventDefault();
                    setVolume(volume + 0.1);
                    break;
                case 'ArrowDown':
                    e.preventDefault();
                    setVolume(volume - 0.1);
                    break;
                case 'm':
                    toggleMute();
                    break;
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => {
            window.removeEventListener('keydown', handleKeyDown);
        };
    }, [audioRef, seekTo, setIsPlaying, volume, setVolume, toggleMute]);
}
