import React, { useEffect } from 'react';

/** Props for useAudioSync hook. */
interface UseAudioSyncProps {
    /** Ref to the audio element. */
    audioRef: React.RefObject<HTMLAudioElement | null>;
    /** Current audio URL. */
    audioUrl: string | null;
    /** Current playback state. */
    isPlaying: boolean;
    /** Playback rate. */
    playbackRate: number;
    /** Function to set the current time in the store. */
    setCurrentTime: (time: number) => void;
    /** Function to set the playing state in the store. */
    setIsPlaying: (isPlaying: boolean) => void;
    /** Function to set the duration. */
    setDuration: (duration: number) => void;
    /** Ref to track the last update time to throttle updates. */
    lastUpdateTimeRef: React.MutableRefObject<number>;
}

/**
 * Hook to synchronize the audio element state with the React state and store.
 * Handles event listeners for time updates, duration changes, etc.
 */
export function useAudioSync({
    audioRef,
    audioUrl,
    isPlaying,
    playbackRate,
    setCurrentTime,
    setIsPlaying,
    setDuration,
    lastUpdateTimeRef
}: UseAudioSyncProps) {

    // Sync audio element play/pause with store state
    useEffect(() => {
        const audio = audioRef.current;
        if (!audio) return;

        if (isPlaying) {
            audio.play().catch(console.error);
        } else {
            audio.pause();
        }
    }, [isPlaying, audioRef]);

    // Sync playback rate
    useEffect(() => {
        if (audioRef.current) {
            audioRef.current.playbackRate = playbackRate;
        }
    }, [playbackRate, audioRef]);

    // Reset duration when audioUrl changes
    useEffect(() => {
        setDuration(0);
        lastUpdateTimeRef.current = 0;
    }, [audioUrl, setDuration, lastUpdateTimeRef]);

    // Handle audio events
    useEffect(() => {
        const audio = audioRef.current;
        if (!audio) return;

        const handleTimeUpdate = () => {
            // This updates the store, triggering re-renders in subscribers (TimeDisplay, SeekSlider)
            // Optimization: Throttle store updates to ~20Hz (every 50ms) to reduce
            // selector execution overhead in subscribed components.
            if (Math.abs(audio.currentTime - lastUpdateTimeRef.current) > 0.05) {
                setCurrentTime(audio.currentTime);
                lastUpdateTimeRef.current = audio.currentTime;
            }
        };

        const updateDuration = () => {
            const d = audio.duration;
            if (d && Number.isFinite(d)) {
                setDuration(d);
            }
        };

        const handleLoadedMetadata = () => {
            updateDuration();
        };

        const handleDurationChange = () => {
            updateDuration();
        };

        const handleEnded = () => {
            setIsPlaying(false);
        };

        const handleError = (e: Event) => {
            console.error('[AudioPlayer] Error event:', e);
        };

        audio.addEventListener('timeupdate', handleTimeUpdate);
        audio.addEventListener('loadedmetadata', handleLoadedMetadata);
        audio.addEventListener('durationchange', handleDurationChange);
        audio.addEventListener('ended', handleEnded);
        audio.addEventListener('error', handleError);

        // Check if metadata is already loaded (HAVE_METADATA = 1)
        if (audio.readyState >= 1) {
            updateDuration();
        }

        return () => {
            audio.removeEventListener('timeupdate', handleTimeUpdate);
            audio.removeEventListener('loadedmetadata', handleLoadedMetadata);
            audio.removeEventListener('durationchange', handleDurationChange);
            audio.removeEventListener('ended', handleEnded);
            audio.removeEventListener('error', handleError);
        };
    }, [setCurrentTime, setIsPlaying, audioUrl, setDuration, lastUpdateTimeRef, audioRef]);
}
