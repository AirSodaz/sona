import { ChevronLeft } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { LlmProvider } from '../../../types/transcript';
import { MODAL_OVERLAY_SELECTOR } from '../../../utils/focusUtils';
import { SettingsPageHeader, SettingsTabContainer } from '../SettingsLayout';
import { AiServicesPage } from './AiServicesPage';
import { ConnectAiDialog } from './ConnectAiDialog';
import { SpeechHome } from './SpeechHome';
import { SpeechModelPage } from './SpeechModelPage';
import './speech-ai.css';
import type { ModelPageScope } from './speechAiLogic';

type Frame = { name: 'home' } | { name: 'model'; scope: ModelPageScope } | { name: 'ai' };

function frameFromEntry(view: string | undefined): Frame {
  switch (view) {
    case 'ai':
      return { name: 'ai' };
    case 'picker':
    case 'live':
      return { name: 'model', scope: 'live' };
    case 'batch':
      return { name: 'model', scope: 'batch' };
    case 'downloads':
      return { name: 'model', scope: 'downloaded' };
    default:
      return { name: 'home' };
  }
}

function modelTitleKey(scope: ModelPageScope): string {
  switch (scope) {
    case 'live':
      return 'settings.speech_ai.model_page_live';
    case 'batch':
      return 'settings.speech_ai.model_page_batch';
    case 'downloaded':
      return 'settings.speech_ai.downloaded';
    case 'embedding-live':
    case 'embedding-batch':
      return 'settings.speech_ai.model_page_embedding';
    case 'segmentation-live':
    case 'segmentation-batch':
      return 'settings.speech_ai.model_page_segmentation';
    default:
      return 'settings.speech_ai.model_page_alignment';
  }
}

export function SettingsSpeechAiTab({
  speechEntry,
}: {
  speechEntry?: { view: string; requestId: number } | null;
}): React.JSX.Element {
  const { t } = useTranslation();
  const [stack, setStack] = useState<Frame[]>(() => [frameFromEntry(speechEntry?.view)]);
  const [connectOpen, setConnectOpen] = useState(false);
  const [editProvider, setEditProvider] = useState<LlmProvider | null>(null);
  const [offline, setOffline] = useState(
    typeof navigator !== 'undefined' ? navigator.onLine === false : false
  );
  const seenEntry = useRef(speechEntry?.requestId);

  if (speechEntry && speechEntry.requestId !== seenEntry.current) {
    seenEntry.current = speechEntry.requestId;
    setStack([frameFromEntry(speechEntry.view)]);
  }

  const frame = stack[stack.length - 1] ?? { name: 'home' };
  const push = (next: Frame) => setStack((current) => [...current, next]);
  const pop = useCallback(
    () =>
      setStack((current) => {
        if (current.length > 1) return current.slice(0, -1);
        if (current[0]?.name !== 'home') return [{ name: 'home' }];
        return current;
      }),
    []
  );

  useEffect(() => {
    const on = () => setOffline(false);
    const off = () => setOffline(true);
    window.addEventListener('online', on);
    window.addEventListener('offline', off);
    return () => {
      window.removeEventListener('online', on);
      window.removeEventListener('offline', off);
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      const overlays = document.querySelectorAll(MODAL_OVERLAY_SELECTOR);
      const top = overlays[overlays.length - 1];
      if (top && !top.classList.contains('settings-overlay')) return;
      if (connectOpen) {
        event.preventDefault();
        event.stopImmediatePropagation();
        setConnectOpen(false);
        return;
      }
      if (frame.name === 'home') return;
      event.preventDefault();
      event.stopImmediatePropagation();
      pop();
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [connectOpen, frame.name, pop]);

  const open = (scope: ModelPageScope | 'ai') => {
    if (scope === 'ai') push({ name: 'ai' });
    else push({ name: 'model', scope });
  };

  const drillTitle =
    frame.name === 'ai'
      ? t('settings.speech_ai.ai_services')
      : frame.name === 'model'
        ? t(modelTitleKey(frame.scope))
        : '';

  return (
    <SettingsTabContainer id="settings-panel-models" ariaLabelledby="settings-tab-models">
      {frame.name === 'home' ? (
        <SettingsPageHeader
          id="settings-speech-ai-title"
          title={t('settings.speech_ai.title')}
          description={t('settings.speech_ai.description')}
        />
      ) : null}
      <div className="speech-ai" id="settings-model-hub">
        {frame.name !== 'home' ? (
          <div className="speech-ai-sticky">
            <button type="button" className="speech-ai-back" onClick={pop}>
              <ChevronLeft size={16} />
              {t('settings.speech_ai.title')}
            </button>
            <h2 className="settings-page-title">{drillTitle}</h2>
            {frame.name === 'model' ? (
              <p className="settings-page-description">
                {frame.scope === 'downloaded'
                  ? t('settings.speech_ai.downloaded_page_desc')
                  : t('settings.speech_ai.model_page_desc')}
              </p>
            ) : null}
          </div>
        ) : null}
        {frame.name === 'home' ? <SpeechHome offline={offline} onOpen={open} /> : null}
        {frame.name === 'model' ? <SpeechModelPage scope={frame.scope} /> : null}
        {frame.name === 'ai' ? (
          <AiServicesPage
            onAdd={() => {
              setEditProvider(null);
              setConnectOpen(true);
            }}
            onEdit={(provider) => {
              setEditProvider(provider);
              setConnectOpen(true);
            }}
          />
        ) : null}
      </div>
      {connectOpen ? (
        <ConnectAiDialog
          initialProvider={editProvider}
          onClose={() => {
            setConnectOpen(false);
            setEditProvider(null);
          }}
        />
      ) : null}
    </SettingsTabContainer>
  );
}
