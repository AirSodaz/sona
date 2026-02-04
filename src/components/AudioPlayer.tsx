import React, { useRef, useEffect, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { formatDisplayTime } from '../utils/exportFormats';
import { PlayFilledIcon, PauseIcon, VolumeIcon, MuteIcon } from './Icons';



// --- Sub-components for Optimization ---

/**
 * Displays the current audio time.
 * Subscribes only to currentTime to prevent full AudioPlayer re-renders.
 */
function TimeDisplayComponent(): React.JSX.Element {
    const currentTime = useTranscriptStore((state) => state.currentTime);
    return <span className="audio-time">{formatDisplayTime(currentTime)}</span>;
}
const TimeDisplay = React.memo(TimeDisplayComponent);

/** Props for the SeekSlider component. */
interface SeekSliderProps {
    /** The total duration of the audio in seconds. */
    duration: number;
    /** Callback fired when the user seeks. */
    onSeek: (time: number) => void;
    /** Accessible label for the slider. */
    seekLabel: string;
}

/**
 * Slider for seeking through audio.
 * Subscribes to currentTime updates.
 */
function SeekSliderComponent({ duration, onSeek, seekLabel }: SeekSliderProps): React.JSX.Element {
    const currentTime = useTranscriptStore((state) => state.currentTime);

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const time = parseFloat(e.target.value);
        onSeek(time);
    };

    return (
        <input
            type="range"
            className="audio-slider"
            min={0}
            max={duration || 0}
            step={0.1}
            value={currentTime}
            onChange={handleChange}
            aria-label={seekLabel}
        />
    );
}
const SeekSlider = React.memo(SeekSliderComponent);

// --- Main Component ---

/** Props for AudioPlayer. */
interface AudioPlayerProps {
    /** Optional CSS class name. */
    className?: string;
}

/**
 * Audio player component with play/pause, seek, and volume controls.
 * Synchronizes with the global transcript store.
 *
 * @param props - Component props.
 * @return The rendered audio player or null if no audio is loaded.
 */
export function AudioPlayer({ className = '' }: AudioPlayerProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const { alert } = useDialogStore();
    const audioRef = useRef<HTMLAudioElement>(null);

    const audioUrl = useTranscriptStore((state) => state.audioUrl);
    // OPTIMIZATION: Do not subscribe to currentTime in the main component.
    // const currentTime = useTranscriptStore((state) => state.currentTime);
    const isPlaying = useTranscriptStore((state) => state.isPlaying);
    const setCurrentTime = useTranscriptStore((state) => state.setCurrentTime);
    const setIsPlaying = useTranscriptStore((state) => state.setIsPlaying);

    const [duration, setDuration] = useState(0);
    const [volume, setVolume] = useState(1);
    const [isMuted, setIsMuted] = useState(false);
    const [prevVolume, setPrevVolume] = useState(1);

    // Sync audio element with store state
    useEffect(() => {
        const audio = audioRef.current;
        if (!audio) return;

        if (isPlaying) {
            audio.play().catch(console.error);
        } else {
            audio.pause();
        }
    }, [isPlaying]);

    // Handle audio events
    useEffect(() => {
        const audio = audioRef.current;
        if (!audio) return;

        const handleTimeUpdate = () => {
            // This updates the store, triggering re-renders in subscribers (TimeDisplay, SeekSlider)
            setCurrentTime(audio.currentTime);
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
    }, [setCurrentTime, setIsPlaying, audioUrl]);

    // Reset duration when audioUrl changes
    useEffect(() => {
        setDuration(0);
    }, [audioUrl]);

    // Expose seek function via store
    const seekTo = useCallback((time: number) => {
        const audio = audioRef.current;
        if (audio) {
            audio.currentTime = time;
            setCurrentTime(time);
        }
    }, [setCurrentTime]);

    // Store seekTo in window for global access (used by TranscriptEditor)
    useEffect(() => {
        (window as unknown as { __audioSeekTo: (time: number) => void }).__audioSeekTo = seekTo;
        return () => {
            delete (window as unknown as { __audioSeekTo?: (time: number) => void }).__audioSeekTo;
        };
    }, [seekTo]);

    const handlePlayPause = () => {
        setIsPlaying(!isPlaying);
    };

    const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const vol = parseFloat(e.target.value);
        setVolume(vol);

        // If user drags slider while muted, unmute
        if (isMuted && vol > 0) {
            setIsMuted(false);
            if (audioRef.current) audioRef.current.muted = false;
        }

        if (audioRef.current) {
            audioRef.current.volume = vol;
        }
    };

    const toggleMute = () => {
        if (isMuted) {
            // Unmute
            setIsMuted(false);
            setVolume(prevVolume);
            if (audioRef.current) {
                audioRef.current.muted = false;
                audioRef.current.volume = prevVolume;
            }
        } else {
            // Mute
            setPrevVolume(volume || 1); // fallback to 1 if current is 0
            setVolume(0);
            setIsMuted(true);
            if (audioRef.current) {
                audioRef.current.muted = true;
                audioRef.current.volume = 0;
            }
        }
    };

    if (!audioUrl) {
        return null;
    }

    return (
        <div className={`audio-player ${className}`}>
            <audio
                ref={audioRef}
                key={audioUrl}
                src={audioUrl}
                preload="metadata"
                onError={(e) => {
                    const error = e.currentTarget.error;
                    console.error('Audio playback error:', error);
                    alert(t('player.error', { error: error?.message || 'Unknown error', code: error?.code }), { variant: 'error' });
                }}
            />

            <div className="audio-controls">
                <button
                    className="btn btn-icon"
                    onClick={handlePlayPause}
                    aria-label={isPlaying ? t('player.pause') : t('player.play')}
                    data-tooltip={isPlaying ? t('player.pause') : t('player.play')}
                >
                    {isPlaying ? <PauseIcon /> : <PlayFilledIcon />}
                </button>
            </div>

            <div className="audio-timeline">
                <TimeDisplay />
                <SeekSlider
                    duration={duration}
                    onSeek={seekTo}
                    seekLabel={t('player.seek')}
                />
                <span className="audio-time">{formatDisplayTime(duration)}</span>
            </div>

            <div className="audio-controls">
                <button
                    className="btn btn-icon"
                    onClick={toggleMute}
                    aria-label={isMuted ? t('player.unmute') : t('player.mute')}
                    aria-pressed={isMuted}
                    data-tooltip={isMuted ? t('player.unmute') : t('player.mute')}
                    data-tooltip-pos="top"
                >
                    {isMuted || volume === 0 ? <MuteIcon /> : <VolumeIcon />}
                </button>
                <input
                    type="range"
                    className="audio-slider audio-slider-volume"
                    min={0}
                    max={1}
                    step={0.01}
                    value={volume}
                    onChange={handleVolumeChange}
                    aria-label={t('player.volume')}
                    data-tooltip={`${Math.round(volume * 100)}%`}
                    data-tooltip-pos="top"
                />
            </div>
        </div>
    );
}

/**
 * Helper function to programmatically seek the audio player from anywhere.
 * Depends on AudioPlayer being mounted.
 *
 * @param time - Time to seek to in seconds.
 */
export function seekAudio(time: number): void {
    const seekFn = (window as unknown as { __audioSeekTo?: (time: number) => void }).__audioSeekTo;
    if (seekFn) {
        seekFn(time);
    }
}

export default AudioPlayer;
