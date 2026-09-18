import type React from 'react';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { SUPPORTED_MEDIA_EXTENSIONS } from '../constants/mediaExtensions';
import {
  isAsrRequestConfigured,
  resolveAsrTranscriptionRequest,
} from '../services/asrConfigService';
import { resolveItemPipeline } from '../services/projectPipeline';
import { invokeTauri } from '../services/tauri/invoke';
import { openDialog } from '../services/tauri/platform/dialog';
import type { Event } from '../services/tauri/platform/events';
import { getCurrentWindow } from '../services/tauri/platform/windows';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useProjectStore } from '../stores/projectStore';
import { logger } from '../utils/logger';
import { getResumeOnboardingStep } from '../utils/onboarding';
import { UploadIcon } from './Icons';
import { QueueGroupedList } from './QueueGroupedList';
import { QueueSummaryBar } from './QueueSummaryBar';
import { TranscriptionOptions } from './TranscriptionOptions';

/** Props for BatchImport. */
interface BatchImportProps {
  /** Optional CSS class name. */
  className?: string;
}

/**
 * Component for batch importing audio files with multi-file queue support.
 *
 * Handles drag-and-drop, file selection, and displays queue sidebar.
 *
 * @param props Component props.
 * @return The batch import UI.
 */
export function BatchImport({ className = '' }: BatchImportProps): React.JSX.Element {
  const showError = useDialogStore((state) => state.showError);
  const [isDragOver, setIsDragOver] = useState(false);
  const { t } = useTranslation();

  // Queue store
  // Optimization: Only subscribe to queue length to avoid re-renders on progress updates
  const hasQueueItems = useBatchQueueStore((state) => state.queueItems.length > 0);
  const addFiles = useBatchQueueStore((state) => state.addFiles);

  // Transcript store
  const config = useConfigStore((state) => state.config);
  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const batchAsrConfigured = isAsrRequestConfigured(
    resolveAsrTranscriptionRequest(config, 'batch')
  );

  const queueFiles = useCallback(
    (files: string[]) => {
      if (!batchAsrConfigured) {
        const asrRequest = resolveAsrTranscriptionRequest(config, 'batch');
        if (asrRequest.engine === 'online') {
          void showError({
            code: 'asr.not_configured',
            messageKey: 'batch.no_model_error',
          });
          return;
        }

        const onboardingStore = useOnboardingStore.getState();
        onboardingStore.reopen(
          getResumeOnboardingStep(config, 'batch_import', onboardingStore.persistedState),
          'batch_import'
        );
        return;
      }

      const pipelineSnapshot = resolveItemPipeline(
        activeProjectId,
        useProjectStore.getState().projects,
        config
      );
      addFiles(files, {
        projectId: activeProjectId,
        pipelineSnapshot,
        resolvedConfigSnapshot: config,
      });
    },
    [activeProjectId, addFiles, batchAsrConfigured, config, showError]
  );

  const handleTauriDrop = useCallback(
    async (payload: unknown): Promise<void> => {
      let files: string[] = [];

      if (Array.isArray(payload)) {
        files = payload as string[];
      } else if (
        payload &&
        typeof payload === 'object' &&
        'paths' in payload &&
        Array.isArray((payload as { paths: unknown }).paths)
      ) {
        files = (payload as { paths: string[] }).paths;
      }

      if (files.length > 0) {
        try {
          const validResults: boolean[] = await invokeTauri('check_media_formats', {
            paths: files,
          });

          const validFiles: string[] = [];
          const invalidFiles: string[] = [];

          files.forEach((filePath, index) => {
            if (validResults[index]) {
              validFiles.push(filePath);
            } else {
              invalidFiles.push(filePath);
            }
          });

          if (invalidFiles.length > 0) {
            void showError({
              code: 'batch.unsupported_format',
              messageKey: 'errors.batch.unsupported_format',
              messageParams: { formats: SUPPORTED_MEDIA_EXTENSIONS.join(', ') },
              showCause: false,
            });
          }

          if (validFiles.length > 0) {
            queueFiles(validFiles);
          }
        } catch (err) {
          logger.error('Failed to validate dropped files', err);
          void showError({
            code: 'batch.unsupported_format',
            messageKey: 'errors.batch.unsupported_format',
            messageParams: { formats: SUPPORTED_MEDIA_EXTENSIONS.join(', ') },
            cause: err,
          });
        }
      } else {
        logger.warn('File drop event received but payload is empty or invalid.');
      }

      setIsDragOver(false);
    },
    [queueFiles, showError]
  );

  const handleClick = useCallback(async (): Promise<void> => {
    try {
      const selected = await openDialog({
        multiple: true,
        filters: [
          {
            name: 'Audio',
            extensions: SUPPORTED_MEDIA_EXTENSIONS.map((ext) => ext.replace('.', '')),
          },
          {
            name: 'All Files',
            extensions: ['*'],
          },
        ],
      });

      if (!selected) {
        return;
      }

      const files = Array.isArray(selected) ? selected : [selected];
      if (files.length > 0) {
        const validResults: boolean[] = await invokeTauri('check_media_formats', { paths: files });

        const validFiles: string[] = [];
        const invalidFiles: string[] = [];

        files.forEach((filePath, index) => {
          if (validResults[index]) {
            validFiles.push(filePath);
          } else {
            invalidFiles.push(filePath);
          }
        });

        if (invalidFiles.length > 0) {
          void showError({
            code: 'batch.unsupported_format',
            messageKey: 'errors.batch.unsupported_format',
            messageParams: { formats: SUPPORTED_MEDIA_EXTENSIONS.join(', ') },
            showCause: false,
          });
        }

        if (validFiles.length > 0) {
          queueFiles(validFiles);
        }
      }
    } catch (err) {
      await showError({
        code: 'batch.file_picker_failed',
        messageKey: 'errors.batch.file_picker_failed',
        cause: err,
      });
    }
  }, [queueFiles, showError]);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
  }, []);

  // Tauri File Drop Event Listener
  useEffect(() => {
    let mounted = true;
    const unlisteners: Array<() => void> = [];

    const setupListeners = async () => {
      const appWindow = getCurrentWindow();

      // Only listen to tauri://drag-drop (Tauri v2)
      const unlistenDrop = await appWindow.listen('tauri://drag-drop', (event: Event<unknown>) => {
        if (mounted) {
          handleTauriDrop(event.payload);
        }
      });
      if (mounted) unlisteners.push(unlistenDrop);

      const unlistenHover = await appWindow.listen('tauri://drag-enter', () => {
        if (mounted) setIsDragOver(true);
      });
      if (mounted) unlisteners.push(unlistenHover);

      const unlistenCancelled = await appWindow.listen('tauri://drag-leave', () => {
        if (mounted) setIsDragOver(false);
      });
      if (mounted) unlisteners.push(unlistenCancelled);
    };

    setupListeners();

    return () => {
      mounted = false;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [handleTauriDrop]);

  const handleDragOver = (e: React.DragEvent): void => {
    e.preventDefault();
    if (!isDragOver) setIsDragOver(true);
  };

  const handleDragEnter = (e: React.DragEvent): void => {
    e.preventDefault();
    setIsDragOver(true);
  };

  const handleDragLeave = (e: React.DragEvent): void => {
    e.preventDefault();
    setIsDragOver(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent): void => {
    if (e.key === 'Enter' || e.key === ' ') {
      void handleClick();
    }
  };

  // Render the queue view when we have items
  if (hasQueueItems) {
    return (
      <div
        className={`batch-import-container batch-import-queue-view ${isDragOver ? 'drag-over' : ''} ${className}`}
        onDrop={handleDrop}
        onDragOver={handleDragOver}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
      >
        <QueueSummaryBar onAddFiles={handleClick} />
        <QueueGroupedList />
        <TranscriptionOptions surface="batch" />
      </div>
    );
  }

  // Initial drop zone view (no queue items)
  return (
    <div className={`batch-import-container ${className}`}>
      <div
        className={`drop-zone drop-zone-wrapper ${isDragOver ? 'drag-over' : ''}`}
        onDrop={handleDrop}
        onDragOver={handleDragOver}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onClick={handleClick}
        onKeyDown={handleKeyDown}
        role="button"
        tabIndex={0}
        aria-label={t('batch.drop_desc')}
      >
        <div className="drop-zone-icon">
          <UploadIcon />
        </div>

        <div className="drop-zone-text">
          <h3>{t('batch.drop_title')}</h3>
          <p>{t('batch.drop_desc')}</p>
        </div>

        <div
          className="btn btn-primary"
          style={{ marginTop: '8px', pointerEvents: 'none' }}
          aria-hidden="true"
        >
          {t('batch.select_file')}
        </div>

        <p className="supported-formats" style={{ marginTop: '8px' }}>
          {t('batch.supports', { formats: SUPPORTED_MEDIA_EXTENSIONS.join(', ') })}
        </p>
      </div>
      <TranscriptionOptions surface="batch" />
    </div>
  );
}
