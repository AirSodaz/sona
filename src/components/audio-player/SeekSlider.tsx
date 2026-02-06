import React, { useRef, useEffect } from 'react';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { formatDisplayTime } from '../../utils/exportFormats';

/** Props for the SeekSlider component. */
export interface SeekSliderProps {
    /** The total duration of the audio in seconds. */
    duration: number;
    /** Callback fired when the user seeks. */
    onSeek: (time: number) => void;
    /** Accessible label for the slider. */
    seekLabel: string;
}

/**
 * Slider for seeking through audio.
 * Subscribes directly to store to avoid React re-renders.
 */
function SeekSliderComponent({ duration, onSeek, seekLabel }: SeekSliderProps): React.JSX.Element {
    const inputRef = useRef<HTMLInputElement>(null);
    const isDragging = useRef(false);

    useEffect(() => {
        if (inputRef.current) {
            const currentTime = useTranscriptStore.getState().currentTime;
            inputRef.current.value = String(currentTime);
            inputRef.current.setAttribute('aria-valuenow', String(currentTime));
            inputRef.current.setAttribute('aria-valuetext', formatDisplayTime(currentTime));
        }

        const unsubscribe = useTranscriptStore.subscribe((state) => {
            const time = state.currentTime;
            // Only update if not currently being dragged
            if (inputRef.current && !isDragging.current) {
                const currentVal = parseFloat(inputRef.current.value);
                // Update only if difference is significant to avoid fighting user input
                if (Math.abs(currentVal - time) > 0.1) {
                    inputRef.current.value = String(time);
                    inputRef.current.setAttribute('aria-valuenow', String(time));
                    inputRef.current.setAttribute('aria-valuetext', formatDisplayTime(time));
                }
            }
        });
        return unsubscribe;
    }, []);

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const time = parseFloat(e.target.value);
        onSeek(time);
    };

    const handleInteractionStart = () => {
        isDragging.current = true;
    };

    const handleInteractionEnd = () => {
        isDragging.current = false;
    };

    return (
        <input
            ref={inputRef}
            type="range"
            className="audio-slider"
            min={0}
            max={duration || 0}
            step={0.1}
            defaultValue={useTranscriptStore.getState().currentTime}
            onChange={handleChange}
            onMouseDown={handleInteractionStart}
            onMouseUp={handleInteractionEnd}
            onTouchStart={handleInteractionStart}
            onTouchEnd={handleInteractionEnd}
            aria-label={seekLabel}
            aria-valuemin={0}
            aria-valuemax={duration || 0}
        />
    );
}

export const SeekSlider = React.memo(SeekSliderComponent);
