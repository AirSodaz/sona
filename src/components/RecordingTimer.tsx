import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

interface RecordingTimerProps {
    isRecording: boolean;
    isPaused: boolean;
}

export function RecordingTimer({ isRecording, isPaused }: RecordingTimerProps): React.ReactElement {
    const { t } = useTranslation();
    const [recordingTime, setRecordingTime] = useState(0);
    const isPausedRef = useRef(isPaused);

    // Update ref when prop changes to keep closure fresh inside interval
    useEffect(() => {
        isPausedRef.current = isPaused;
    }, [isPaused]);

    // Format recording time
    function formatTime(seconds: number): string {
        const mins = Math.floor(seconds / 60);
        const secs = seconds % 60;
        return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
    }

    useEffect(() => {
        let interval: number | undefined;

        if (isRecording) {
            interval = window.setInterval(() => {
                if (!isPausedRef.current) {
                    setRecordingTime(t => t + 1);
                }
            }, 1000);
        } else {
            setRecordingTime(0);
        }

        return () => {
            if (interval) clearInterval(interval);
        };
    }, [isRecording]);

    return (
        <div
            className="recording-timer"
            role="timer"
            aria-label={t('live.timer_label')}
            aria-live="off"
            style={{
                visibility: isRecording ? 'visible' : 'hidden'
            }}
        >
            {formatTime(recordingTime)}
        </div>
    );
}
