import React, { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

interface CaptionUpdatePayload {
    text: string;
    isFinal: boolean;
}

interface LockStatePayload {
    locked: boolean;
}

export function LiveCaptionWindow() {
    const [text, setText] = useState<string>('Ready for captioning...');
    const [isLocked, setIsLocked] = useState<boolean>(true); // Default to locked

    useEffect(() => {
        const setupListeners = async () => {
            const currentWindow = getCurrentWindow();

            const unlistenUpdate = await currentWindow.listen<CaptionUpdatePayload>('caption-update', (event) => {
                setText(event.payload.text);
            });

            const unlistenLock = await currentWindow.listen<LockStatePayload>('caption-lock-state', (event) => {
                setIsLocked(event.payload.locked);
            });

            return () => {
                unlistenUpdate();
                unlistenLock();
            };
        };

        const cleanupPromise = setupListeners();

        return () => {
            cleanupPromise.then(cleanup => cleanup && cleanup());
        };
    }, []);

    // Styles for the caption window
    const containerStyle: React.CSSProperties = {
        width: '100%',
        height: '100vh',
        backgroundColor: 'rgba(0, 0, 0, 0.6)', // Semi-transparent dark background
        color: '#ffffff',
        display: 'flex',
        flexDirection: 'column',
        justifyContent: 'center',
        alignItems: 'center',
        padding: '20px',
        boxSizing: 'border-box',
        textAlign: 'center',
        fontSize: '24px',
        fontWeight: 500,
        textShadow: '0 2px 4px rgba(0,0,0,0.8)',
        overflow: 'hidden',
        userSelect: 'none',
        borderRadius: '8px',
        cursor: isLocked ? 'default' : 'move',
        transition: 'background-color 0.3s ease',
    };

    const hintStyle: React.CSSProperties = {
        position: 'absolute',
        top: 8,
        right: 12,
        fontSize: '12px',
        opacity: 0.8,
        pointerEvents: 'none',
        textTransform: 'uppercase',
        letterSpacing: '0.5px',
        fontWeight: 'bold',
        color: '#ffd700', // Gold color for visibility
    };

    return (
        <div
            style={containerStyle}
            // Add drag region only when unlocked
            {...(!isLocked ? { 'data-tauri-drag-region': true } : {})}
        >
            <div style={{ pointerEvents: 'none', maxWidth: '90%' }}>
                {text}
            </div>

            {!isLocked && (
                <div style={hintStyle}>
                    Unlocked (Drag to move)
                </div>
            )}
        </div>
    );
}
