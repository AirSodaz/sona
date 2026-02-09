import React, { useRef, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { formatDisplayTime } from '../utils/exportFormats';
import { PlayFilledIcon, PauseIcon, VolumeIcon, MuteIcon } from './Icons';
import { TimeDisplay } from './audio-player/TimeDisplay';
import { SeekSlider } from './audio-player/SeekSlider';
import { useAudioShortcuts } from '../hooks/useAudioShortcuts';
import { useAudioVolume } from '../hooks/useAudioVolume';
import { useAudioSync } from '../hooks/useAudioSync';

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
    const isPlaying = useTranscriptStore((state) => state.isPlaying);
    const setCurrentTime = useTranscriptStore((state) => state.setCurrentTime);
    const setIsPlaying = useTranscriptStore((state) => state.setIsPlaying);
    const seekRequest = useTranscriptStore((state) => state.seekRequest);
    const requestSeek = useTranscriptStore((state) => state.requestSeek);

    const [duration, setDuration] = useState(0);
    const [playbackRate, setPlaybackRate] = useState(1.0);

    const {
        volume,
        isMuted,
        handleVolumeChange,
        toggleMute,
        setVolume
    } = useAudioVolume(audioRef);

    // Sync audio state and events
    useAudioSync({
        audioRef,
        audioUrl,
        isPlaying,
        playbackRate,
        setCurrentTime,
        setIsPlaying,
        setDuration,
        lastUpdateTimeRef: lastUpdateTime
    });

    // Handle seek request from store
    useEffect(() => {
        if (seekRequest && audioRef.current) {
            // Only update if difference is significant to avoid stutter
            if (Math.abs(audioRef.current.currentTime - seekRequest.time) > 0.001) {
                audioRef.current.currentTime = seekRequest.time;
                lastUpdateTime.current = seekRequest.time;
            }
        }
    }, [seekRequest]);

    // Initialize shortcuts hook
    useAudioShortcuts({
        audioRef,
        seekTo: requestSeek,
        setIsPlaying,
        volume,
        setVolume,
        toggleMute
    });

    const handlePlayPause = () => {
        setIsPlaying(!isPlaying);
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
                    onSeek={requestSeek}
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
                    aria-pressed={!!isMuted}
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

export default AudioPlayer;
