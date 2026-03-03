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
            amplitudeRef.current += (targetAmplitude - amplitudeRef.current) * 0.04; // Slower, graceful interpolation
            phaseRef.current += 0.03; // Slower, fluid phase movement

            const { width, height } = canvas;
            const maxWaveHeight = height * 0.8;
            const amplitudePx = amplitudeRef.current * maxWaveHeight;

            ctx.clearRect(0, 0, width, height);

            // Create soft vertical gradient with more visible color
            if (typeof ctx.createLinearGradient === 'function') {
                const gradient = ctx.createLinearGradient(0, height - maxWaveHeight, 0, height);
                gradient.addColorStop(0, 'rgba(224, 62, 62, 0.6)'); // Muted Red (Recording color)
                gradient.addColorStop(1, 'rgba(224, 62, 62, 0.1)');
                ctx.fillStyle = gradient;
            } else {
                // Fallback for limited canvas mock environments in tests
                ctx.fillStyle = 'rgba(224, 62, 62, 0.6)';
            }

            if (typeof ctx.beginPath === 'function') {
                ctx.beginPath();
            }
            if (typeof ctx.moveTo === 'function') {
                ctx.moveTo(0, height);
            }

            const cycles = 2.0;
            for (let x = 0; x <= width; x += 2) {
                const t = x / width;

                // Combine sine waves for an undulating organic look
                const wave1 = Math.sin((t * cycles * Math.PI * 2) + phaseRef.current);
                const wave2 = Math.sin((t * (cycles * 0.5) * Math.PI * 2) + phaseRef.current * 1.5) * 0.5;

                // Normalize wave combo (-1.5 to 1.5) to a roughly 0 to 1 range envelope
                const normalizedWave = (wave1 + wave2 + 1.5) / 3.0;

                // Minimum height so a baseline wave is always visible
                const minHeight = height * 0.05;
                const y = height - minHeight - (normalizedWave * amplitudePx);

                if (typeof ctx.lineTo === 'function') {
                    ctx.lineTo(x, y);
                }
            }

            if (typeof ctx.lineTo === 'function') {
                ctx.lineTo(width, height);
            }
            if (typeof ctx.closePath === 'function') {
                ctx.closePath();
            }

            if (typeof ctx.fill === 'function') {
                ctx.fill();
            }
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
