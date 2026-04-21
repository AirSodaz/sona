import React from 'react';
import { useTranslation } from 'react-i18next';

interface RecordingTimerProps {
    elapsedMs: number;
    isRecording: boolean;
}

export function RecordingTimer({ elapsedMs, isRecording }: RecordingTimerProps): React.ReactElement {
    const { t } = useTranslation();

    // Format recording time
    function formatTime(milliseconds: number): string {
        const seconds = Math.max(0, Math.floor(milliseconds / 1000));
        const mins = Math.floor(seconds / 60);
        const secs = seconds % 60;
        return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
    }

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
            {formatTime(elapsedMs)}
        </div>
    );
}
