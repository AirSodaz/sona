import React, { useRef, useEffect, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { formatDisplayTime } from '../utils/exportFormats';
import { PlayFilledIcon, PauseIcon, VolumeIcon, MuteIcon } from './Icons';
import { TimeDisplay } from './audio-player/TimeDisplay';
import { SeekSlider } from './audio-player/SeekSlider';

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
    const lastUpdateTime = useRef(0);

    const audioUrl = useTranscriptStore((state) => state.audioUrl);
    // OPTIMIZATION: Do not subscribe to currentTime in the main component.
    // const currentTime = useTranscriptStore((state) => state.currentTime);
    const isPlaying = useTranscriptStore((state) => state.isPlaying);
    const setCurrentTime = useTranscriptStore((state) => state.setCurrentTime);
    const setIsPlaying = useTranscriptStore((state) => state.setIsPlaying);
    const triggerSeek = useTranscriptStore((state) => state.triggerSeek);

    const [duration, setDuration] = useState(0);
    const [volume, setVolume] = useState(1);
    const [isMuted, setIsMuted] = useState(false);
    const [prevVolume, setPrevVolume] = useState(1);
    const [playbackRate, setPlaybackRate] = useState(1.0);

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

    // Sync playback rate
    useEffect(() => {
        if (audioRef.current) {
            audioRef.current.playbackRate = playbackRate;
        }
    }, [playbackRate]);

    // Handle audio events
    useEffect(() => {
        const audio = audioRef.current;
        if (!audio) return;

        const handleTimeUpdate = () => {
            // This updates the store, triggering re-renders in subscribers (TimeDisplay, SeekSlider)
            // Optimization: Throttle store updates to ~20Hz (every 50ms) to reduce
            // selector execution overhead in subscribed components.
            if (Math.abs(audio.currentTime - lastUpdateTime.current) > 0.05) {
                setCurrentTime(audio.currentTime);
                lastUpdateTime.current = audio.currentTime;
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
    }, [setCurrentTime, setIsPlaying, audioUrl]);

    // Reset duration when audioUrl changes
    useEffect(() => {
        setDuration(0);
        lastUpdateTime.current = 0;
    }, [audioUrl]);

    // Expose seek function via store
    const seekTo = useCallback((time: number) => {
        const audio = audioRef.current;
        if (audio) {
            audio.currentTime = time;
            setCurrentTime(time);
            lastUpdateTime.current = time;
            triggerSeek();
        }
    }, [setCurrentTime, triggerSeek]);

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

    // Keyboard shortcuts
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
                    setVolume(Math.min(1, volume + 0.1));
                    if (audio.muted) {
                        audio.muted = false;
                        setIsMuted(false);
                    }
                    audio.volume = Math.min(1, volume + 0.1);
                    break;
                case 'ArrowDown':
                    e.preventDefault();
                    setVolume(Math.max(0, volume - 0.1));
                    audio.volume = Math.max(0, volume - 0.1);
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
    }, [seekTo, setIsPlaying, volume, toggleMute]);

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
                    className="btn btn-icon btn-text"
                    onClick={() => {
                        const speeds = [0.5, 0.8, 1.0, 1.25, 1.5, 2.0, 3.0];
                        const currentIndex = speeds.indexOf(playbackRate);
                        const nextSpeed = speeds[(currentIndex + 1) % speeds.length];
                        setPlaybackRate(nextSpeed);
                    }}
                    aria-label={`${t('player.speed')} ${playbackRate}x`}
                    data-tooltip={t('player.speed')}
                    data-tooltip-pos="top"
                    style={{ minWidth: '3ch', fontSize: '0.85rem', fontWeight: 500 }}
                >
                    {playbackRate}x
                </button>
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
                    aria-valuenow={volume}
                    aria-valuemin={0}
                    aria-valuemax={1}
                    aria-valuetext={`${Math.round(volume * 100)}%`}
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
