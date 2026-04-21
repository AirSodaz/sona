import { type CSSProperties, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Mic } from 'lucide-react';
import '../styles/index.css';
import { logger } from '../utils/logger';
import { useAuxWindowState } from '../hooks/useAuxWindowState';
import {
    DEFAULT_VOICE_TYPING_OVERLAY_STATE,
    VOICE_TYPING_EVENT_TEXT,
    VOICE_TYPING_WINDOW_LABEL,
} from '../services/voiceTypingWindowService';

const baseContainerStyle = {
    display: 'flex',
    alignItems: 'center',
    gap: '10px',
    padding: '10px 14px',
    margin: '4px',
    borderRadius: '16px',
    fontSize: '14px',
    boxShadow: '0 10px 30px rgba(15, 23, 42, 0.22)',
    maxWidth: '90vw',
    transition:
        'background 120ms ease, border-color 120ms ease, box-shadow 120ms ease, color 120ms ease',
} satisfies CSSProperties;

export function VoiceTypingOverlay() {
    const { t } = useTranslation();
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

    const { phase, text } = overlayState;
    const isSegment = phase === 'segment' && text.trim().length > 0;
    const isError = phase === 'error';
    const displayText =
        isSegment || isError
            ? text
            : phase === 'preparing'
                ? t('common.preparing')
                : t('common.listening');

    const containerStyle: CSSProperties = isSegment
        ? {
            ...baseContainerStyle,
            background: 'rgba(255, 255, 255, 0.94)',
            color: '#111827',
            border: '1px solid rgba(15, 23, 42, 0.12)',
            boxShadow: '0 12px 30px rgba(15, 23, 42, 0.18)',
        }
        : isError
            ? {
                ...baseContainerStyle,
                background: 'rgba(127, 29, 29, 0.92)',
                color: '#ffffff',
                border: '1px solid rgba(248, 113, 113, 0.35)',
            }
            : {
                ...baseContainerStyle,
                background: 'rgba(15, 23, 42, 0.78)',
                backdropFilter: 'blur(10px)',
                WebkitBackdropFilter: 'blur(10px)',
                color: 'white',
                border: '1px solid rgba(255,255,255,0.12)',
            };

    const indicatorStyle: CSSProperties = isSegment
        ? {
            width: '4px',
            alignSelf: 'stretch',
            borderRadius: '999px',
            background: 'linear-gradient(180deg, #34d399 0%, #22c55e 100%)',
        }
        : {
            width: '8px',
            height: '8px',
            borderRadius: '999px',
            background: isError ? '#fca5a5' : '#4ade80',
            boxShadow: isError
                ? '0 0 0 4px rgba(248, 113, 113, 0.16)'
                : '0 0 0 4px rgba(74, 222, 128, 0.16)',
            flexShrink: 0,
        };

    return (
        <div
            style={{
                width: '100vw',
                height: '100vh',
                display: 'flex',
                alignItems: 'flex-start',
                justifyContent: 'flex-start',
                background: 'transparent',
                overflow: 'hidden',
            }}
        >
            <div style={containerStyle}>
                {isSegment ? (
                    <div style={indicatorStyle} />
                ) : (
                    <>
                        <Mic
                            size={16}
                            className={isError ? undefined : 'animate-pulse'}
                            style={{
                                color: isError ? '#fecaca' : '#4ade80',
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
