import React, { useRef, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { formatDisplayTime } from '../utils/exportFormats';

// Icons
const PlayIcon = () => (
    <svg viewBox="0 0 24 24" fill="currentColor">
        <path d="M8 5v14l11-7z" />
    </svg>
);

const PauseIcon = () => (
    <svg viewBox="0 0 24 24" fill="currentColor">
        <rect x="6" y="4" width="4" height="16" />
        <rect x="14" y="4" width="4" height="16" />
    </svg>
);

const VolumeIcon = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
        <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
        <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
    </svg>
);

// --- Sub-components for Optimization ---

// TimeDisplay subscribes to currentTime to update only the text
const TimeDisplay = React.memo(() => {
    const currentTime = useTranscriptStore((state) => state.currentTime);
    return <span className="audio-time">{formatDisplayTime(currentTime)}</span>;
});

// SeekSlider subscribes to currentTime to update the slider
// It receives stable seekTo and duration
interface SeekSliderProps {
    duration: number;
    onSeek: (time: number) => void;
    seekLabel: string;
}

const SeekSlider = React.memo<SeekSliderProps>(({ duration, onSeek, seekLabel }) => {
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
});

// --- Main Component ---

interface AudioPlayerProps {
    className?: string;
}

export const AudioPlayer: React.FC<AudioPlayerProps> = ({ className = '' }) => {
    const { t } = useTranslation();
    const { alert } = useDialogStore();
    const audioRef = useRef<HTMLAudioElement>(null);

    const audioUrl = useTranscriptStore((state) => state.audioUrl);
    // OPTIMIZATION: Do not subscribe to currentTime in the main component.
    // const currentTime = useTranscriptStore((state) => state.currentTime);
    const isPlaying = useTranscriptStore((state) => state.isPlaying);
    const setCurrentTime = useTranscriptStore((state) => state.setCurrentTime);
    const setIsPlaying = useTranscriptStore((state) => state.setIsPlaying);

    const [duration, setDuration] = React.useState(0);
    const [volume, setVolume] = React.useState(1);

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
        if (audioRef.current) {
            audioRef.current.volume = vol;
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
                    {isPlaying ? <PauseIcon /> : <PlayIcon />}
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
                <VolumeIcon />
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
};

// Helper function for external seek access
export const seekAudio = (time: number) => {
    const seekFn = (window as unknown as { __audioSeekTo?: (time: number) => void }).__audioSeekTo;
    if (seekFn) {
        seekFn(time);
    }
};

export default AudioPlayer;
