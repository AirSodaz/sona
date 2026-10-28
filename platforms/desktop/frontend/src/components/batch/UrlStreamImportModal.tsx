import { Globe } from 'lucide-react';
import type React from 'react';
import { useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { Checkbox } from '../Checkbox';
import { Modal } from '../Modal';

export interface UrlStreamImportModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function UrlStreamImportModal({
  isOpen,
  onClose,
}: UrlStreamImportModalProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const [urlInput, setUrlInput] = useState('');
  const [alias, setAlias] = useState('');
  const [retainAudio, setRetainAudio] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const errorId = useId();

  const textareaId = useId();
  const aliasId = useId();

  const urls = urlInput
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0);

  const isSingleUrl = urls.length === 1;

  const handleModalClose = () => {
    setUrlInput('');
    setAlias('');
    setRetainAudio(false);
    setErrorMessage(null);
    onClose();
  };

  const handleSubmit = (e?: React.FormEvent) => {
    if (e) {
      e.preventDefault();
    }
    setErrorMessage(null);

    if (urls.length === 0) {
      setErrorMessage(
        t('batch.stream_modal_empty_error', {
          defaultValue: 'Please enter at least one audio or video stream URL.',
        })
      );
      return;
    }

    const invalidUrls = urls.filter((url) => !/^(https?|rtmp|rtsp):\/\/[^\s/?#]\S*$/i.test(url));

    if (invalidUrls.length > 0) {
      setErrorMessage(
        t('batch.stream_modal_invalid_error', {
          defaultValue: 'URLs must begin with http://, https://, rtmp://, or rtsp://.',
        })
      );
      return;
    }

    const uniqueUrls = Array.from(new Set(urls));
    useBatchQueueStore.getState().addStreamItems(uniqueUrls, {
      alias: isSingleUrl && alias.trim() ? alias.trim() : undefined,
      retainAudio,
    });
    handleModalClose();
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={handleModalClose}
      initialFocusRef={textareaRef}
      title={
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <Globe size={18} />
          <span>
            {t('batch.import_stream_title', {
              defaultValue: 'Import Network Audio/Video Stream',
            })}
          </span>
        </div>
      }
      size="md"
      footer={
        <div
          style={{
            display: 'flex',
            justifyContent: 'flex-end',
            gap: '8px',
            width: '100%',
          }}
        >
          <button type="button" className="btn btn-secondary-soft" onClick={handleModalClose}>
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </button>
          <button
            type="submit"
            form={`${textareaId}-form`}
            className="btn btn-primary"
            disabled={urls.length === 0}
          >
            {t('batch.add_to_queue', { defaultValue: 'Add to Queue' })}
          </button>
        </div>
      }
    >
      <form
        id={`${textareaId}-form`}
        onSubmit={handleSubmit}
        onKeyDown={(e) => {
          if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
            e.preventDefault();
            handleSubmit();
          }
        }}
        style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}
      >
        <div>
          <label
            htmlFor={textareaId}
            style={{
              display: 'block',
              fontSize: '13px',
              fontWeight: 500,
              marginBottom: '6px',
              color: 'var(--color-text-primary)',
            }}
          >
            {t('batch.stream_urls_label', {
              defaultValue: 'Network Stream URLs (one per line)',
            })}
          </label>
          <textarea
            ref={textareaRef}
            id={textareaId}
            rows={4}
            value={urlInput}
            onChange={(e) => {
              setUrlInput(e.target.value);
              if (errorMessage) setErrorMessage(null);
            }}
            aria-invalid={Boolean(errorMessage)}
            aria-describedby={errorMessage ? errorId : undefined}
            placeholder={`https://example.com/podcast.mp3\nhttps://live.example.com/audio-stream.m3u8\nrtsp://192.168.1.1:554/live`}
            style={{
              width: '100%',
              padding: '10px',
              fontSize: '13px',
              fontFamily: 'monospace',
              borderRadius: 'var(--radius-md, 6px)',
              border: '1px solid var(--color-border, #333)',
              backgroundColor: 'var(--color-bg-subtle, rgba(0,0,0,0.2))',
              color: 'var(--color-text-primary)',
              resize: 'vertical',
              boxSizing: 'border-box',
            }}
            onKeyDown={(e) => {
              if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
                e.preventDefault();
                handleSubmit();
              }
            }}
          />
          <div
            style={{
              fontSize: '12px',
              color: 'var(--color-text-muted)',
              marginTop: '4px',
            }}
          >
            {t('batch.stream_urls_hint', {
              defaultValue:
                'Transcribes directly from the network stream; processed audio slices are discarded immediately without consuming permanent disk space.',
            })}
          </div>
        </div>

        {isSingleUrl && (
          <div>
            <label
              htmlFor={aliasId}
              style={{
                display: 'block',
                fontSize: '13px',
                fontWeight: 500,
                marginBottom: '6px',
                color: 'var(--color-text-primary)',
              }}
            >
              {t('batch.stream_alias_label', {
                defaultValue: 'Custom Name (Optional)',
              })}
            </label>
            <input
              id={aliasId}
              type="text"
              value={alias}
              onChange={(e) => setAlias(e.target.value)}
              placeholder={t('batch.stream_alias_placeholder', {
                defaultValue: 'e.g., AI Podcast Episode 42',
              })}
              style={{
                width: '100%',
                padding: '8px 10px',
                fontSize: '13px',
                borderRadius: 'var(--radius-md, 6px)',
                border: '1px solid var(--color-border, #333)',
                backgroundColor: 'var(--color-bg-subtle, rgba(0,0,0,0.2))',
                color: 'var(--color-text-primary)',
                boxSizing: 'border-box',
              }}
            />
          </div>
        )}

        <div
          style={{
            padding: '10px 12px',
            borderRadius: 'var(--radius-md, 6px)',
            backgroundColor: 'var(--color-bg-subtle, rgba(255,255,255,0.03))',
            border: '1px solid var(--color-border, rgba(255,255,255,0.08))',
          }}
        >
          <Checkbox
            checked={retainAudio}
            onChange={setRetainAudio}
            label={t('batch.stream_retain_audio', {
              defaultValue: 'Keep Transcoded Audio Copy (allows workbench playback after saving)',
            })}
          />
          <div
            style={{
              fontSize: '12px',
              color: 'var(--color-text-muted)',
              marginTop: '4px',
              marginLeft: '26px',
            }}
          >
            {t('batch.stream_retain_audio_desc', {
              defaultValue:
                'When unchecked, audio is kept only in memory during processing and discarded immediately with zero local disk footprint.',
            })}
          </div>
        </div>

        {errorMessage && (
          <div
            id={errorId}
            role="alert"
            aria-live="polite"
            style={{
              color: 'var(--color-error, #ef4444)',
              fontSize: '13px',
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
            }}
          >
            <span aria-hidden="true">⚠️</span>
            <span>{errorMessage}</span>
          </div>
        )}
      </form>
    </Modal>
  );
}
