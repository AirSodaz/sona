import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle,
  CheckCircle2,
  Info,
  Loader2,
  RefreshCw,
  Stethoscope,
  TriangleAlert,
  X,
  XCircle,
} from 'lucide-react';
import { diagnosticsService } from '../services/diagnosticsService';
import { requestMicrophonePermission } from '../services/audioDeviceService';
import { voiceTypingService } from '../services/voiceTypingService';
import type {
  DiagnosticAction,
  DiagnosticCheck,
  DiagnosticOverviewCard,
  DiagnosticStatus,
  DiagnosticsSnapshot,
} from '../types/diagnostics';
import type { SettingsTab } from '../hooks/useSettingsLogic';
import { normalizeError } from '../utils/errorUtils';
import './PanelModal.css';
import './DiagnosticsModal.css';

interface DiagnosticsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenSettingsTab: (tab: SettingsTab) => void;
  onRunFirstRunSetup: () => void;
}

function formatScannedAt(scannedAt: string, t: (key: string, options?: Record<string, unknown>) => string): string {
  if (!scannedAt) {
    return t('settings.diagnostics.scanned_unknown', {
      defaultValue: 'Scan time unavailable',
    });
  }

  return new Date(scannedAt).toLocaleString();
}

function getStatusIcon(status: DiagnosticStatus): React.ReactNode {
  switch (status) {
    case 'ready':
      return <CheckCircle2 size={14} />;
    case 'warning':
      return <TriangleAlert size={14} />;
    case 'missing':
      return <AlertTriangle size={14} />;
    case 'failed':
      return <XCircle size={14} />;
    case 'info':
    default:
      return <Info size={14} />;
  }
}

function getStatusClass(status: DiagnosticStatus): string {
  return `diagnostics-status-badge status-${status}`;
}

function getStatusLabel(
  status: DiagnosticStatus,
  t: (key: string, options?: Record<string, unknown>) => string,
): string {
  return t(`settings.diagnostics.status_${status}`, {
    defaultValue: status,
  });
}

function DiagnosticCard({
  item,
  onAction,
  busyAction,
  t,
}: {
  item: DiagnosticOverviewCard;
  onAction: (action: DiagnosticAction) => Promise<void>;
  busyAction: string | null;
  t: (key: string, options?: Record<string, unknown>) => string;
}): React.JSX.Element {
  return (
    <article className={`diagnostics-overview-card status-${item.status}`}>
      <div className="diagnostics-overview-top">
        <div className={getStatusClass(item.status)}>
          {getStatusIcon(item.status)}
          <span>{getStatusLabel(item.status, t)}</span>
        </div>
      </div>
      <div className="diagnostics-overview-title">{item.title}</div>
      <div className="diagnostics-overview-description">{item.description}</div>
      {item.action ? (
        <button
          type="button"
          className="btn btn-secondary panel-modal-inline-action diagnostics-inline-action"
          onClick={() => void onAction(item.action!)}
          disabled={busyAction === item.action.kind}
        >
          {busyAction === item.action.kind ? <Loader2 size={14} className="queue-icon-spin" /> : null}
          {item.action.label}
        </button>
      ) : null}
    </article>
  );
}

function DiagnosticCheckRow({
  check,
  onAction,
  busyAction,
  t,
}: {
  check: DiagnosticCheck;
  onAction: (action: DiagnosticAction) => Promise<void>;
  busyAction: string | null;
  t: (key: string, options?: Record<string, unknown>) => string;
}): React.JSX.Element {
  return (
    <div className="diagnostics-check-row">
      <div className="diagnostics-check-main">
        <div className="diagnostics-check-header">
          <div className="diagnostics-check-title">{check.title}</div>
          <div className={getStatusClass(check.status)}>
            {getStatusIcon(check.status)}
            <span>{getStatusLabel(check.status, t)}</span>
          </div>
        </div>
        <div className="diagnostics-check-description">{check.description}</div>
        {check.meta ? <div className="diagnostics-check-meta">{check.meta}</div> : null}
      </div>
      {check.action ? (
        <button
          type="button"
          className="btn btn-secondary panel-modal-inline-action diagnostics-inline-action"
          onClick={() => void onAction(check.action!)}
          disabled={busyAction === check.action.kind}
        >
          {busyAction === check.action.kind ? <Loader2 size={14} className="queue-icon-spin" /> : null}
          {check.action.label}
        </button>
      ) : null}
    </div>
  );
}

export function DiagnosticsModal({
  isOpen,
  onClose,
  onOpenSettingsTab,
  onRunFirstRunSetup,
}: DiagnosticsModalProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const [snapshot, setSnapshot] = useState<DiagnosticsSnapshot | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);

  const loadSnapshot = useCallback(async () => {
    setIsLoading(true);
    setLoadError(null);
    try {
      const nextSnapshot = await diagnosticsService.collectSnapshot(t);
      setSnapshot(nextSnapshot);
    } catch (error) {
      setLoadError(normalizeError(error).message);
    } finally {
      setIsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    queueMicrotask(() => {
      void loadSnapshot();
    });
  }, [isOpen, loadSnapshot]);

  const handleAction = useCallback(async (action: DiagnosticAction) => {
    setBusyAction(action.kind);
    try {
      switch (action.kind) {
        case 'open_settings':
          onOpenSettingsTab(action.settingsTab);
          return;
        case 'run_first_run_setup':
          onRunFirstRunSetup();
          return;
        case 'open_log_folder':
          await invoke('open_log_folder');
          break;
        case 'request_microphone_permission':
          await requestMicrophonePermission();
          break;
        case 'retry_voice_typing_warmup':
          await voiceTypingService.retryWarmup();
          break;
        default:
          break;
      }

      await loadSnapshot();
    } catch (error) {
      setLoadError(normalizeError(error).message);
    } finally {
      setBusyAction(null);
    }
  }, [loadSnapshot, onOpenSettingsTab, onRunFirstRunSetup]);

  const scannedAtLabel = useMemo(() => (
    snapshot
      ? formatScannedAt(snapshot.scannedAt, t)
      : t('settings.diagnostics.scanned_unknown', {
          defaultValue: 'Scan time unavailable',
        })
  ), [snapshot, t]);

  if (!isOpen) {
    return null;
  }

  return (
    <div className="settings-overlay panel-modal-overlay diagnostics-overlay" onClick={onClose}>
      <div
        className="panel-modal-shell diagnostics-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="diagnostics-title"
      >
        <div className="panel-modal-header diagnostics-header">
          <div className="panel-modal-header-copy diagnostics-header-copy">
            <div className="panel-modal-badge diagnostics-badge">
              <Stethoscope size={16} />
              <span>{t('settings.diagnostics.badge', { defaultValue: 'Diagnostics' })}</span>
            </div>
            <h2 id="diagnostics-title">
              {t('settings.diagnostics.title', { defaultValue: 'Model & Environment Diagnostics' })}
            </h2>
            <p>
              {t('settings.diagnostics.description', {
                defaultValue: 'Review the local transcription path, packaged runtime dependencies, and the clearest next fix when something is off.',
              })}
            </p>
          </div>
          <div className="panel-modal-header-controls">
            <div className="panel-modal-toolbar diagnostics-header-actions">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => void loadSnapshot()}
                disabled={isLoading}
              >
                {isLoading ? <Loader2 size={14} className="queue-icon-spin" /> : <RefreshCw size={14} />}
                {t('settings.diagnostics.refresh', { defaultValue: 'Refresh' })}
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => void handleAction({
                  kind: 'open_log_folder',
                  label: t('settings.about_open_logs', { defaultValue: 'Open Log Folder' }),
                })}
              >
                {t('settings.about_open_logs', { defaultValue: 'Open Log Folder' })}
              </button>
            </div>
            <button
              type="button"
              className="btn btn-icon panel-modal-close"
              onClick={onClose}
              aria-label={t('common.close', { defaultValue: 'Close' })}
            >
              <X size={18} />
            </button>
          </div>
        </div>

        <div className="panel-modal-meta-row diagnostics-meta-row">
          <span className="panel-modal-meta-label diagnostics-meta-label">
            {t('settings.diagnostics.last_scanned', { defaultValue: 'Last scanned' })}
          </span>
          <span>{scannedAtLabel}</span>
        </div>

        {loadError ? (
          <div className="diagnostics-error-banner" role="alert">
            <strong>{t('settings.diagnostics.error_title', { defaultValue: 'Diagnostics unavailable' })}</strong>
            <span>{loadError}</span>
          </div>
        ) : null}

        <div className="panel-modal-content diagnostics-content">
          {isLoading && !snapshot ? (
            <div className="diagnostics-loading-state">
              <Loader2 size={18} className="queue-icon-spin" />
              <span>{t('settings.diagnostics.loading', { defaultValue: 'Scanning your local environment...' })}</span>
            </div>
          ) : null}

          {snapshot ? (
            <>
              <section className="diagnostics-overview-grid" aria-label={t('settings.diagnostics.overview', { defaultValue: 'Overview' })}>
                {snapshot.overview.map((item) => (
                  <DiagnosticCard
                    key={item.id}
                    item={item}
                    onAction={handleAction}
                    busyAction={busyAction}
                    t={t}
                  />
                ))}
              </section>

              {snapshot.sections.map((section) => (
                <section className="panel-modal-section diagnostics-section" key={section.id}>
                  <div className="panel-modal-section-header diagnostics-section-header">
                    <div className="panel-modal-section-title diagnostics-section-title">{section.title}</div>
                    {section.description ? (
                      <div className="panel-modal-section-description diagnostics-section-description">{section.description}</div>
                    ) : null}
                  </div>
                  <div className="panel-modal-section-body diagnostics-section-body">
                    {section.checks.map((check) => (
                      <DiagnosticCheckRow
                        key={check.id}
                        check={check}
                        onAction={handleAction}
                        busyAction={busyAction}
                        t={t}
                      />
                    ))}
                  </div>
                </section>
              ))}
            </>
          ) : null}
        </div>
      </div>
    </div>
  );
}
