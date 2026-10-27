import type React from 'react';
import { useId, useState } from 'react';
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

  const textareaId = useId();
  const aliasId = useId();

  const urls = urlInput
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0);

  const isSingleUrl = urls.length === 1;

  const handleSubmit = (e?: React.FormEvent) => {
    if (e) {
      e.preventDefault();
    }
    setErrorMessage(null);

    if (urls.length === 0) {
      setErrorMessage(
        t('batch.stream_modal_empty_error', {
          defaultValue: '请输入至少一个音视频网络流链接。',
        })
      );
      return;
    }

    const invalidUrls = urls.filter(
      (url) =>
        !url.startsWith('http://') &&
        !url.startsWith('https://') &&
        !url.startsWith('rtmp://') &&
        !url.startsWith('rtsp://')
    );

    if (invalidUrls.length > 0) {
      setErrorMessage(
        t('batch.stream_modal_invalid_error', {
          defaultValue: '链接必须以 http://, https://, rtmp:// 或 rtsp:// 开头。',
        })
      );
      return;
    }

    useBatchQueueStore.getState().addStreamItems(urls, {
      alias: isSingleUrl && alias.trim() ? alias.trim() : undefined,
      retainAudio,
    });

    setUrlInput('');
    setAlias('');
    setRetainAudio(false);
    onClose();
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span>🌐</span>
          <span>
            {t('batch.import_stream_title', {
              defaultValue: '导入网络音视频链接',
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
          <button type="button" className="btn btn-secondary-soft" onClick={onClose}>
            {t('common.cancel', { defaultValue: '取消' })}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => handleSubmit()}
            disabled={urls.length === 0}
          >
            {t('batch.add_to_queue', { defaultValue: '加入队列' })}
          </button>
        </div>
      }
    >
      <form
        onSubmit={handleSubmit}
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
              defaultValue: '网络音视频链接 (支持多行批量输入)',
            })}
          </label>
          <textarea
            id={textareaId}
            rows={4}
            value={urlInput}
            onChange={(e) => {
              setUrlInput(e.target.value);
              if (errorMessage) setErrorMessage(null);
            }}
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
            autoFocus
          />
          <div
            style={{
              fontSize: '12px',
              color: 'var(--color-text-muted)',
              marginTop: '4px',
            }}
          >
            {t('batch.stream_urls_hint', {
              defaultValue: '直接通过网络流转录，完成转录的音频切片随用随弃，不占用长期磁盘空间。',
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
                defaultValue: '自定义名称 (可选)',
              })}
            </label>
            <input
              id={aliasId}
              type="text"
              value={alias}
              onChange={(e) => setAlias(e.target.value)}
              placeholder={t('batch.stream_alias_placeholder', {
                defaultValue: '例如：AI 播客第 42 期',
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
              defaultValue: '保留转码音频副本 (保存后可在工作台回放)',
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
              defaultValue: '未勾选时，音频只在内存中滑动处理后即时销毁，零本地磁盘占用。',
            })}
          </div>
        </div>

        {errorMessage && (
          <div
            style={{
              color: 'var(--color-error, #ef4444)',
              fontSize: '13px',
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
            }}
          >
            <span>⚠️</span>
            <span>{errorMessage}</span>
          </div>
        )}
      </form>
    </Modal>
  );
}
