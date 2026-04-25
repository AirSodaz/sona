import { getCurrentWindow } from '@tauri-apps/api/window';
import { type CSSProperties, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Mic } from 'lucide-react';
import '../styles/index.css';
import { logger } from '../utils/logger';
import { useAuxWindowState } from '../hooks/useAuxWindowState';
import { useAuxWindowTheme } from '../hooks/useAuxWindowTheme';
import {
    DEFAULT_VOICE_TYPING_OVERLAY_STATE,
    VOICE_TYPING_EVENT_TEXT,
    VOICE_TYPING_WINDOW_LABEL,
    VOICE_TYPING_WINDOW_WIDTH,
} from '../services/voiceTypingWindowService';

const OVERLAY_ROOT_PADDING = {
    top: 4,
    right: 4,
    bottom: 20,
    left: 4,
};

const baseContainerStyle = {
    display: 'flex',
    alignItems: 'center',
    gap: '10px',
    padding: '10px 14px',
    borderRadius: '16px',
    fontSize: '14px',
    maxWidth: `${VOICE_TYPING_WINDOW_WIDTH - OVERLAY_ROOT_PADDING.left - OVERLAY_ROOT_PADDING.right}px`,
    transition:
        'background 120ms ease, border-color 120ms ease, box-shadow 120ms ease, color 120ms ease',
} satisfies CSSProperties;

async function resizeVoiceTypingWindow(rootElement: HTMLDivElement | null) {
    if (!rootElement) {
        return;
    }

    const totalHeight = Math.ceil(rootElement.getBoundingClientRect().height);
    if (totalHeight <= 0) {
        return;
    }

    try {
        const currentWindow = getCurrentWindow();
        const factor = await currentWindow.scaleFactor();
        const size = await currentWindow.innerSize();
        const targetPhysicalHeight = Math.ceil(totalHeight * factor);
        const targetPhysicalWidth = Math.ceil(VOICE_TYPING_WINDOW_WIDTH * factor);

        if (
            Math.abs(size.height - targetPhysicalHeight) <= 1 &&
            Math.abs(size.width - targetPhysicalWidth) <= 1
        ) {
            return;
        }

        const { PhysicalSize } = await import('@tauri-apps/api/dpi');
        await currentWindow.setSize(
            new PhysicalSize(targetPhysicalWidth, targetPhysicalHeight)
        );
    } catch (error) {
        logger.error('[VoiceTypingOverlay] Failed to resize window:', error);
    }
}

export function VoiceTypingOverlay() {
    const { t } = useTranslation();
    const rootRef = useRef<HTMLDivElement>(null);
    const resolvedTheme = useAuxWindowTheme();
    const overlayState = useAuxWindowState({
        label: VOICE_TYPING_WINDOW_LABEL,
        eventName: VOICE_TYPING_EVENT_TEXT,
        defaultState: DEFAULT_VOICE_TYPING_OVERLAY_STATE,
        onStateApplied: (payload, source) => {
            void logger.info('[VoiceTypingOverlay] Applied overlay state', {
                source,
                sessionId: payload.sessionId,
                revision: payload.revision,
                phase: payload.phase,
                segmentId: payload.segmentId ?? null,
                isFinal: payload.isFinal ?? null,
                textLength: payload.text.length,
            });
        },
    });

    useEffect(() => {
        const previousDocumentBackground = document.documentElement.style.background;
        const previousBodyBackground = document.body.style.background;

        document.documentElement.style.background = 'transparent';
        document.body.style.background = 'transparent';

        return () => {
            document.documentElement.style.background = previousDocumentBackground;
            document.body.style.background = previousBodyBackground;
        };
    }, []);

    useEffect(() => {
        if (!rootRef.current) {
            return;
        }

        const observer = new ResizeObserver(() => {
            void resizeVoiceTypingWindow(rootRef.current);
        });

        observer.observe(rootRef.current);
        void resizeVoiceTypingWindow(rootRef.current);

        return () => {
            observer.disconnect();
        };
    }, []);

    const { phase, text } = overlayState;
    const isSegment = phase === 'segment' && text.trim().length > 0;
    const isError = phase === 'error';
    const displayText =
        isSegment || isError
            ? text
            : phase === 'preparing'
                ? t('common.preparing')
                : t('common.listening');

    useEffect(() => {
        if (phase !== 'segment') {
            return;
        }

        void logger.info('[VoiceTypingOverlay] Rendered segment state', {
            sessionId: overlayState.sessionId,
            revision: overlayState.revision,
            textLength: text.length,
            renderedPhase: isSegment ? 'segment' : 'listening',
        });
    }, [isSegment, overlayState.revision, overlayState.sessionId, phase, text]);

    useEffect(() => {
        void resizeVoiceTypingWindow(rootRef.current);
    }, [displayText, phase, resolvedTheme]);

    let containerStyle: CSSProperties;
    let indicatorStyle: CSSProperties;

    if (isSegment) {
        containerStyle = {
            ...baseContainerStyle,
            background: 'var(--color-bg-elevated)',
            color: 'var(--color-text-primary)',
            border: '1px solid var(--color-border-hover)',
            boxShadow: resolvedTheme === 'dark'
                ? '0 16px 32px rgba(0, 0, 0, 0.36)'
                : 'var(--shadow-xl)',
        };

        indicatorStyle = {
            width: '4px',
            alignSelf: 'stretch',
            borderRadius: '999px',
            background: 'linear-gradient(180deg, #34d399 0%, #22c55e 100%)',
        };
    } else if (isError) {
        containerStyle = {
            ...baseContainerStyle,
            background: 'rgba(127, 29, 29, 0.92)',
            color: '#ffffff',
            border: '1px solid rgba(248, 113, 113, 0.35)',
        };

        indicatorStyle = {
            width: '8px',
            height: '8px',
            borderRadius: '999px',
            background: '#fca5a5',
            boxShadow: '0 0 0 4px rgba(248, 113, 113, 0.16)',
            flexShrink: 0,
        };
    } else {
        containerStyle = {
            ...baseContainerStyle,
            background: 'var(--color-bg-elevated)',
            color: 'var(--color-text-primary)',
            border: '1px solid var(--color-border-hover)',
            boxShadow: resolvedTheme === 'dark'
                ? '0 16px 32px rgba(0, 0, 0, 0.36)'
                : 'var(--shadow-xl)',
        };

        indicatorStyle = {
            width: '8px',
            height: '8px',
            borderRadius: '999px',
            background: '#4ade80',
            boxShadow: '0 0 0 4px rgba(74, 222, 128, 0.16)',
            flexShrink: 0,
        };
    }

    let micColor = '#4ade80';
    let micClass: string | undefined = 'animate-pulse';
    if (isError) {
        micColor = '#fecaca';
        micClass = undefined;
    }

    return (
        <div
            data-testid="voice-typing-overlay-root"
            ref={rootRef}
            style={{
                display: 'flex',
                alignItems: 'flex-start',
                justifyContent: 'flex-start',
                background: 'transparent',
                overflow: 'visible',
                width: 'fit-content',
                height: 'fit-content',
                padding: `${OVERLAY_ROOT_PADDING.top}px ${OVERLAY_ROOT_PADDING.right}px ${OVERLAY_ROOT_PADDING.bottom}px ${OVERLAY_ROOT_PADDING.left}px`,
            }}
        >
            <div data-testid="voice-typing-bubble" style={containerStyle}>
                {isSegment ? (
                    <div style={indicatorStyle} />
                ) : (
                    <>
                        <Mic
                            size={16}
                            className={micClass}
                            style={{
                                color: micColor,
                                flexShrink: 0,
                            }}
                        />
                        <div style={indicatorStyle} />
                    </>
                )}
                <span
                    style={{
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        fontWeight: isSegment ? 600 : 500,
                        letterSpacing: isSegment ? '0.01em' : 'normal',
                    }}
                >
                    {displayText}
                </span>
            </div>
        </div>
    );
}
