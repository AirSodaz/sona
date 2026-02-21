import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from '../Dropdown';

interface SettingsMicrophoneTabProps {
    microphoneId: string;
    setMicrophoneId: (id: string) => void;
}

export function SettingsMicrophoneTab({
    microphoneId,
    setMicrophoneId
}: SettingsMicrophoneTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const [devices, setDevices] = useState<{ label: string; value: string }[]>([]);
    const [permissionGranted, setPermissionGranted] = useState(false);

    const canvasRef = useRef<HTMLCanvasElement>(null);
    const audioContextRef = useRef<AudioContext | null>(null);
    const analyserRef = useRef<AnalyserNode | null>(null);
    const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null);
    const animationRef = useRef<number>(0);
    const streamRef = useRef<MediaStream | null>(null);

    // Enumerate devices
    useEffect(() => {
        let isMounted = true;

        async function getDevices() {
            try {
                // Check if we have permission first, otherwise labels might be empty
                // We'll try to get a temporary stream to trigger permission prompt if needed
                // or just rely on existing permissions.
                // If labels are empty, we might need to prompt.

                const devs = await navigator.mediaDevices.enumerateDevices();
                const audioInputs = devs.filter(d => d.kind === 'audioinput');

                // If no labels, we might need to request permission
                const hasLabels = audioInputs.some(d => d.label.length > 0);
                if (!hasLabels) {
                    try {
                        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
                        stream.getTracks().forEach(t => t.stop());
                        // Try again
                        const newDevs = await navigator.mediaDevices.enumerateDevices();
                         const newAudioInputs = newDevs.filter(d => d.kind === 'audioinput');
                         if (isMounted) {
                             formatAndSetDevices(newAudioInputs);
                             setPermissionGranted(true);
                         }
                    } catch (err) {
                        console.warn('Microphone permission denied or error', err);
                    }
                } else {
                     if (isMounted) {
                         formatAndSetDevices(audioInputs);
                         setPermissionGranted(true);
                     }
                }

            } catch (err) {
                console.error('Error enumerating devices:', err);
            }
        }

        function formatAndSetDevices(inputs: MediaDeviceInfo[]) {
            const options = [
                { label: t('settings.mic_auto'), value: 'default' },
                ...inputs.map(d => ({
                    label: d.label || `Microphone ${d.deviceId.slice(0, 5)}...`,
                    value: d.deviceId
                }))
            ];
            // Remove duplicates (sometimes default and deviceId are same physical device, but we want distinct options in UI?
            // Usually 'default' is separate.
            // But if we have multiple entries with same deviceId, filter them.
            // enumerateDevices returns unique deviceIds usually.

            // Deduplicate by value just in case
            const uniqueOptions = options.filter((opt, index, self) =>
                index === self.findIndex((t) => (
                    t.value === opt.value
                ))
            );

            setDevices(uniqueOptions);
        }

        getDevices();

        return () => {
            isMounted = false;
        };
    }, [t]);


    // Visualizer Logic
    useEffect(() => {
        let isMounted = true;
        // Start visualization for the selected microphone
        // This runs whenever microphoneId changes or component mounts

        startVisualizer(microphoneId, () => isMounted);

        return () => {
            isMounted = false;
            stopVisualizer();
        };
    }, [microphoneId]);

    async function startVisualizer(deviceId: string, checkMounted: () => boolean) {
        stopVisualizer(); // Stop previous if any

        try {
            const constraints: MediaStreamConstraints = {
                audio: deviceId === 'default'
                    ? true
                    : { deviceId: { exact: deviceId } }
            };

            const stream = await navigator.mediaDevices.getUserMedia(constraints);

            if (!checkMounted()) {
                stream.getTracks().forEach(t => t.stop());
                return;
            }

            streamRef.current = stream;

            const AudioContextClass = window.AudioContext || (window as any).webkitAudioContext;
            const audioCtx = new AudioContextClass();
            audioContextRef.current = audioCtx;

            const analyser = audioCtx.createAnalyser();
            analyser.fftSize = 256;
            analyserRef.current = analyser;

            const source = audioCtx.createMediaStreamSource(stream);
            source.connect(analyser);
            sourceRef.current = source;

            draw();
        } catch (err) {
            console.error('Error starting visualizer:', err);
        }
    }

    function stopVisualizer() {
        if (animationRef.current) {
            cancelAnimationFrame(animationRef.current);
            animationRef.current = 0;
        }
        if (sourceRef.current) {
            sourceRef.current.disconnect();
            sourceRef.current = null;
        }
        if (analyserRef.current) {
            analyserRef.current.disconnect();
            analyserRef.current = null;
        }
        if (audioContextRef.current) {
            audioContextRef.current.close();
            audioContextRef.current = null;
        }
        if (streamRef.current) {
            streamRef.current.getTracks().forEach(t => t.stop());
            streamRef.current = null;
        }
    }

    function draw() {
        const canvas = canvasRef.current;
        const analyser = analyserRef.current;

        if (!canvas || !analyser) return;

        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const bufferLength = analyser.frequencyBinCount;
        const dataArray = new Uint8Array(bufferLength);

        // Cache gradients if possible, or create on the fly (it's fast enough usually)

        const drawLoop = () => {
            animationRef.current = requestAnimationFrame(drawLoop);

            analyser.getByteFrequencyData(dataArray);

            ctx.clearRect(0, 0, canvas.width, canvas.height);

            const barWidth = (canvas.width / bufferLength) * 2.5;
            let x = 0;

            for (let i = 0; i < bufferLength; i++) {
                const value = dataArray[i];
                const barHeight = (value / 255) * canvas.height;

                // Simple gray gradient
                ctx.fillStyle = `rgb(${value + 50}, ${value + 50}, ${value + 50})`;
                ctx.fillRect(x, canvas.height - barHeight, barWidth, barHeight);

                x += barWidth + 1;
            }
        };

        drawLoop();
    }


    return (
        <div
            className="settings-group"
            role="tabpanel"
            id="settings-panel-microphone"
            aria-labelledby="settings-tab-microphone"
            tabIndex={0}
        >
            <div className="settings-item">
                <label htmlFor="settings-mic-select" className="settings-label">
                    {t('settings.microphone_selection')}
                </label>
                <div style={{ display: 'flex', alignItems: 'center', gap: '16px', width: '100%' }}>
                    <div style={{ flex: 1, maxWidth: 300 }}>
                        <Dropdown
                            id="settings-mic-select"
                            value={microphoneId}
                            onChange={setMicrophoneId}
                            options={devices}
                        />
                    </div>

                    <div style={{
                        width: '120px',
                        height: '36px',
                        backgroundColor: 'var(--color-bg-secondary)',
                        borderRadius: '4px',
                        overflow: 'hidden',
                        border: '1px solid var(--color-border)'
                    }}>
                        <canvas
                            ref={canvasRef}
                            width={120}
                            height={36}
                            style={{ display: 'block', width: '100%', height: '100%' }}
                        />
                    </div>
                </div>
            </div>

             <div className="settings-hint">
                 {t('settings.mic_auto_hint', { defaultValue: 'Select which microphone to use for recording.' })}
             </div>

        </div>
    );
}
