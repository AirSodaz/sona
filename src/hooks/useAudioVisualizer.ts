import { useRef, useCallback, useEffect } from 'react';

interface UseAudioVisualizerProps {
    canvasRef: React.RefObject<HTMLCanvasElement>;
    analyserRef: React.RefObject<AnalyserNode | null>;
    isPaused: boolean;
}

/**
 * Hook for managing the audio visualizer canvas.
 *
 * @param props The hook properties.
 * @return An object containing functions to control the visualizer.
 */
export function useAudioVisualizer({ canvasRef, analyserRef, isPaused }: UseAudioVisualizerProps): {
    startVisualizer: () => void;
    stopVisualizer: () => void;
} {
    const animationRef = useRef<number>(0);
    const isPausedRef = useRef(isPaused);

    useEffect(() => {
        isPausedRef.current = isPaused;
    }, [isPaused]);

    const drawVisualizer = useCallback(() => {
        if (animationRef.current) {
            window.cancelAnimationFrame(animationRef.current);
        }

        const canvas = canvasRef.current;
        const analyser = analyserRef.current;
        if (!canvas || !analyser) return;

        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const bufferLength = analyser.frequencyBinCount;
        const dataArray = new Uint8Array(bufferLength);

        // Cache for gradients: 256 possible byte values
        const gradients = new Array<CanvasGradient | undefined>(256);
        let cachedHeight = canvas.height;

        const draw = () => {
            // Optimization: Stop the loop if paused
            if (isPausedRef.current) {
                return;
            }

            animationRef.current = window.requestAnimationFrame(draw);

            // Invalidate cache if height changes (e.g. resize)
            if (canvas.height !== cachedHeight) {
                gradients.fill(undefined);
                cachedHeight = canvas.height;
            }

            analyser.getByteFrequencyData(dataArray);

            ctx.clearRect(0, 0, canvas.width, canvas.height); // Use CSS background

            const barWidth = (canvas.width / bufferLength) * 2.5;
            let x = 0;

            for (let i = 0; i < bufferLength; i++) {
                const value = dataArray[i];
                const barHeight = (value / 255) * canvas.height * 0.8;

                if (!gradients[value]) {
                    const gradient = ctx.createLinearGradient(0, canvas.height, 0, canvas.height - barHeight);
                    gradient.addColorStop(0, '#37352f'); // Notion Black
                    gradient.addColorStop(1, '#787774'); // Notion Gray
                    gradients[value] = gradient;
                }

                ctx.fillStyle = gradients[value]!;
                ctx.fillRect(x, canvas.height - barHeight, barWidth - 1, barHeight);

                x += barWidth;
            }
        };

        draw();
    }, [canvasRef, analyserRef]);

    const stopVisualizer = useCallback(() => {
        if (animationRef.current) {
            window.cancelAnimationFrame(animationRef.current);
            animationRef.current = 0;
        }

        const canvas = canvasRef.current;
        const ctx = canvas?.getContext('2d');
        if (canvas && ctx) {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
        }
    }, [canvasRef]);

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            if (animationRef.current && typeof window.cancelAnimationFrame === 'function') {
                window.cancelAnimationFrame(animationRef.current);
            }
        };
    }, []);

    return {
        startVisualizer: drawVisualizer,
        stopVisualizer
    };
}
