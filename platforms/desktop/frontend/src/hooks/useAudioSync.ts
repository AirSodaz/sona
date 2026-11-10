import type React from 'react';
import { useEffect, useRef } from 'react';
import { useBoundSessionId } from '../contexts/TranscriptSessionContext';
import { useTranscriptStore } from '../stores/transcriptStore';
import { logger } from '../utils/logger';

/** Props for useAudioSync hook. */
interface UseAudioSyncProps {
  /** Ref to the audio element. */
  audioRef: React.RefObject<HTMLAudioElement | null>;
  /** Current audio URL. */
  audioUrl: string | null;
  /** Whether the host shell owning this player is active. Defaults to true. */
  isActive?: boolean;
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
  lastUpdateTimeRef: { current: number };
}

/**
 * Hook to synchronize the audio element state with the React state and store.
 * Handles event listeners for time updates, duration changes, etc.
 */
export function useAudioSync({
  audioRef,
  audioUrl,
  isActive = true,
  isPlaying,
  playbackRate,
  setCurrentTime,
  setIsPlaying,
  setDuration,
  lastUpdateTimeRef,
}: UseAudioSyncProps) {
  const boundSessionId = useBoundSessionId();
  const restoredKeyRef = useRef<string | null>(null);

  // Sync audio element play/pause with store state & host visibility
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;

    if (isPlaying && isActive) {
      audio.play().catch(logger.error);
    } else {
      audio.pause();
    }
  }, [isActive, isPlaying, audioRef]);

  // Pause session playback flag cleanly when the host tab switches away without
  // clearing the saved `currentTime`, so returning restores & resumes accurately.
  useEffect(() => {
    if (!isActive && isPlaying) {
      setIsPlaying(false);
    }
  }, [isActive, isPlaying, setIsPlaying]);

  // Sync playback rate
  useEffect(() => {
    if (audioRef.current) {
      audioRef.current.playbackRate = playbackRate;
    }
  }, [playbackRate, audioRef]);

  // Reset duration when audioUrl or bound session changes
  useEffect(() => {
    if (boundSessionId && audioUrl !== undefined) {
      setDuration(0);
      lastUpdateTimeRef.current = 0;
      restoredKeyRef.current = null;
    }
  }, [audioUrl, boundSessionId, setDuration, lastUpdateTimeRef]);

  // Handle audio events
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || audioUrl === undefined) return;

    const handleTimeUpdate = () => {
      // This updates the store, triggering re-renders in subscribers (TimeDisplay, SeekSlider)
      // Optimization: Throttle store updates to ~20Hz (every 50ms) to reduce
      // selector execution overhead in subscribed components.
      if (Math.abs(audio.currentTime - lastUpdateTimeRef.current) > 0.05) {
        setCurrentTime(audio.currentTime);
        lastUpdateTimeRef.current = audio.currentTime;
      }
    };

    const restoreSavedTime = () => {
      const targetKey = `${boundSessionId}:${audioUrl ?? ''}`;
      if (restoredKeyRef.current === targetKey) {
        return;
      }
      restoredKeyRef.current = targetKey;
      const savedTime = useTranscriptStore.getState().sessions[boundSessionId]?.currentTime ?? 0;
      if (savedTime > 0 && Math.abs(audio.currentTime - savedTime) > 0.05) {
        audio.currentTime = savedTime;
        lastUpdateTimeRef.current = savedTime;
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
      restoreSavedTime();
    };

    const handleDurationChange = () => {
      updateDuration();
    };

    const handleEnded = () => {
      setIsPlaying(false);
    };

    const handleError = (e: Event) => {
      logger.error('[AudioPlayer] Error event:', e);
    };

    audio.addEventListener('timeupdate', handleTimeUpdate);
    audio.addEventListener('loadedmetadata', handleLoadedMetadata);
    audio.addEventListener('durationchange', handleDurationChange);
    audio.addEventListener('ended', handleEnded);
    audio.addEventListener('error', handleError);

    // Check if metadata is already loaded (HAVE_METADATA = 1)
    if (audio.readyState >= 1) {
      updateDuration();
      restoreSavedTime();
    }

    return () => {
      audio.removeEventListener('timeupdate', handleTimeUpdate);
      audio.removeEventListener('loadedmetadata', handleLoadedMetadata);
      audio.removeEventListener('durationchange', handleDurationChange);
      audio.removeEventListener('ended', handleEnded);
      audio.removeEventListener('error', handleError);
    };
  }, [
    boundSessionId,
    setCurrentTime,
    setIsPlaying,
    audioUrl,
    setDuration,
    lastUpdateTimeRef,
    audioRef,
  ]);
}
