/**
 * Draws the audio visualizer on the provided canvas.
 *
 * @param canvas - The canvas element.
 * @param analyser - The audio analyser node.
 * @param isPausedRef - Ref to check if visualization should be paused.
 * @return Cleanup function to cancel the animation frame.
 */
export function drawAudioVisualizer(
    canvas: HTMLCanvasElement,
    analyser: AnalyserNode,
    isPausedRef: { current: boolean }
): () => void {
    if (!canvas || !analyser) return () => {};

    const ctx = canvas.getContext('2d');
    if (!ctx) return () => {};

    const bufferLength = analyser.frequencyBinCount;
    const dataArray = new Uint8Array(bufferLength);
    let animationId = 0;

    // Cache for gradients: 256 possible byte values
    const gradients = new Array<CanvasGradient | undefined>(256);
    let cachedHeight = canvas.height;

    const draw = () => {
        // Optimization: Stop the loop if paused
        if (isPausedRef.current) {
            return;
        }

        if (typeof window.requestAnimationFrame === 'function') {
            animationId = window.requestAnimationFrame(draw);
        }

        // Invalidate cache if height changes (e.g. resize)
        if (canvas.height !== cachedHeight) {
            gradients.fill(undefined);
            cachedHeight = canvas.height;
        }

        analyser.getByteFrequencyData(dataArray);

        ctx.clearRect(0, 0, canvas.width, canvas.height);

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

    return () => {
        if (animationId && typeof window.cancelAnimationFrame === 'function') {
            window.cancelAnimationFrame(animationId);
        }
    };
}
