import { useState, RefObject } from 'react';

interface UseAudioVolumeReturn {
    volume: number;
    isMuted: boolean;
    handleVolumeChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
    toggleMute: () => void;
    setVolume: (volume: number) => void;
    setIsMuted: (isMuted: boolean) => void;
}

/**
 * Hook to manage audio volume and mute state.
 *
 * @param audioRef - Reference to the audio element.
 * @return Volume control methods and state.
 */
export function useAudioVolume(audioRef: RefObject<HTMLAudioElement | null>): UseAudioVolumeReturn {
    const [volume, setVolumeState] = useState(1);
    const [isMuted, setIsMuted] = useState(false);
    const [prevVolume, setPrevVolume] = useState(1);

    const updateVolume = (newVolume: number) => {
        const clamped = Math.max(0, Math.min(1, newVolume));
        setVolumeState(clamped);

        if (audioRef.current) {
            audioRef.current.volume = clamped;
        }

        // Auto-unmute if volume is increased
        if (isMuted && clamped > 0) {
            setIsMuted(false);
            if (audioRef.current) audioRef.current.muted = false;
        }
    };

    const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const vol = parseFloat(e.target.value);
        updateVolume(vol);
    };

    const toggleMute = () => {
        if (isMuted) {
            // Unmute
            setIsMuted(false);
            setVolumeState(prevVolume);
            if (audioRef.current) {
                audioRef.current.muted = false;
                audioRef.current.volume = prevVolume;
            }
        } else {
            // Mute
            setPrevVolume(volume || 1);
            setVolumeState(0);
            setIsMuted(true);
            if (audioRef.current) {
                audioRef.current.muted = true;
                audioRef.current.volume = 0;
            }
        }
    };

    return {
        volume,
        isMuted,
        handleVolumeChange,
        toggleMute,
        setVolume: updateVolume,
        setIsMuted
    };
}
