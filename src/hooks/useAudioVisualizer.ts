import { useRef, useCallback, useEffect } from 'react';

interface UseAudioVisualizerProps {
    canvasRef: React.RefObject<HTMLCanvasElement | null>;
    peakLevelRef: React.MutableRefObject<number>;
    isPaused: boolean;
}

export function useAudioVisualizer({ canvasRef, peakLevelRef, isPaused }: UseAudioVisualizerProps): {
    startVisualizer: () => void;
    stopVisualizer: () => void;
} {
    const animationRef = useRef<number>(0);
    const isPausedRef = useRef(isPaused);
    const amplitudeRef = useRef(0);
    const phaseRef = useRef(0);

    useEffect(() => {
        isPausedRef.current = isPaused;
    }, [isPaused]);

    const drawVisualizer = useCallback(() => {
        if (animationRef.current) {
            window.cancelAnimationFrame(animationRef.current);
        }

        const canvas = canvasRef.current;
        if (!canvas) return;

        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const draw = () => {
            if (isPausedRef.current) {
                return;
            }

            animationRef.current = window.requestAnimationFrame(draw);

            const targetAmplitude = Math.max(0.02, Math.min(1, peakLevelRef.current));
            amplitudeRef.current += (targetAmplitude - amplitudeRef.current) * 0.08;
            phaseRef.current += 0.06;

            const { width, height } = canvas;
            const centerY = height / 2;
            const maxWaveHeight = height * 0.42;
            const amplitudePx = amplitudeRef.current * maxWaveHeight;

            ctx.clearRect(0, 0, width, height);
            ctx.lineWidth = 2;
            ctx.strokeStyle = '#4b5563';
            ctx.beginPath();

            const cycles = 2.5;
            for (let x = 0; x <= width; x += 2) {
                const t = x / width;
                const y =
                    centerY +
                    Math.sin((t * cycles * Math.PI * 2) + phaseRef.current) * amplitudePx +
                    Math.sin((t * (cycles * 0.5) * Math.PI * 2) + phaseRef.current * 1.5) * (amplitudePx * 0.35);
                if (x === 0) {
                    ctx.moveTo(x, y);
                } else {
                    ctx.lineTo(x, y);
                }
            }

            ctx.stroke();
        };

        draw();
    }, [canvasRef, peakLevelRef]);

    const stopVisualizer = useCallback(() => {
        if (animationRef.current) {
            if (typeof window.cancelAnimationFrame === 'function') {
                window.cancelAnimationFrame(animationRef.current);
            }
            animationRef.current = 0;
        }
        amplitudeRef.current = 0;

        const canvas = canvasRef.current;
        const ctx = canvas?.getContext('2d');
        if (canvas && ctx) {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
        }
    }, [canvasRef]);

    useEffect(() => {
        return () => {
            if (animationRef.current) {
                if (typeof window.cancelAnimationFrame === 'function') {
                    window.cancelAnimationFrame(animationRef.current);
                }
            }
        };
    }, []);

    return {
        startVisualizer: drawVisualizer,
        stopVisualizer
    };
}
