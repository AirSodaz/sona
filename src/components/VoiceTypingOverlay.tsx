import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { VOICE_TYPING_EVENT_TEXT } from '../services/voiceTypingWindowService';
import { Mic } from 'lucide-react';
import '../styles/index.css';

export function VoiceTypingOverlay() {
    const { t } = useTranslation();
    const [text, setText] = useState(t('common.listening'));

    useEffect(() => {
        const unlistenPromise = listen<{ text: string }>(VOICE_TYPING_EVENT_TEXT, (event) => {
            const newText = event.payload.text;
            if (newText === '' || !newText) {
                setText(t('common.listening'));
            } else {
                setText(newText);
            }
        });

        return () => {
            unlistenPromise.then(unlisten => unlisten());
        };
    }, [t]);

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

    return (
        <div style={{
            width: '100vw',
            height: '100vh',
            display: 'flex',
            alignItems: 'flex-start',
            justifyContent: 'flex-start',
            background: 'transparent',
            overflow: 'hidden',
        }}>
            <div style={{
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
                background: 'rgba(0, 0, 0, 0.7)',
                backdropFilter: 'blur(10px)',
                WebkitBackdropFilter: 'blur(10px)',
                color: 'white',
                padding: '8px 16px',
                margin: '4px',
                borderRadius: '999px',
                fontSize: '14px',
                boxShadow: '0 4px 6px rgba(0,0,0,0.1)',
                maxWidth: '90%',
                border: '1px solid rgba(255,255,255,0.1)'
            }}>
                <Mic size={16} className="animate-pulse" style={{ color: '#4ade80' }} />
                <span style={{ 
                    whiteSpace: 'nowrap', 
                    overflow: 'hidden', 
                    textOverflow: 'ellipsis' 
                }}>
                    {text}
                </span>
            </div>
        </div>
    );
}
