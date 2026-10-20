import { CheckCircle2, Languages, Stethoscope, Terminal, XCircle } from 'lucide-react';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { APP_LANGUAGE_OPTIONS } from '../../constants/appLanguages';
import { shimService } from '../../services/shimService';
import { useSetConfig, useUIConfig } from '../../stores/configStore';
import type { AppLanguagePreference, AppLogLevel, UIConfig } from '../../types/config';
import type { DesktopShimStatus, ShimChannel } from '../../types/shim';
import { APP_LOG_LEVELS, normalizeLogLevel } from '../../utils/logLevel';
import { markSettingsPerf } from '../../utils/settingsPerf';
import { Dropdown } from '../Dropdown';
import { GeneralIcon } from '../Icons';
import { Switch } from '../Switch';
import {
  SettingsItem,
  SettingsPageHeader,
  SettingsSection,
  SettingsTabContainer,
} from './SettingsLayout';

interface SettingsGeneralTabProps {
  isVisible?: boolean;
  isPrewarming?: boolean;
  onOpenDiagnostics?: () => void;
}

type FontValue = NonNullable<UIConfig['font']>;

function getFontFamily(fontValue: string): string {
  switch (fontValue) {
    case 'mono':
      return 'monospace';
    case 'serif':
      return 'serif';
    default:
      return 'inherit';
  }
}

export function SettingsGeneralTab({
  isVisible = true,
  isPrewarming = false,
  onOpenDiagnostics,
}: SettingsGeneralTabProps): React.JSX.Element {
  const { t } = useTranslation();
  const config = useUIConfig();
  const updateConfig = useSetConfig();
  const [shimStatus, setShimStatus] = React.useState<DesktopShimStatus | null>(null);
  const [activeAction, setActiveAction] = React.useState<
    'install' | 'uninstall' | 'channel' | null
  >(null);
  const [isStatusLoading, setIsStatusLoading] = React.useState(false);
  const isShimLoading = activeAction !== null || isStatusLoading;
  const [shimActionError, setShimActionError] = React.useState<string | null>(null);
  const isMountedRef = React.useRef(true);
  const fetchSeqRef = React.useRef(0);

  const loadShimStatus = React.useCallback(async () => {
    const seq = ++fetchSeqRef.current;
    if (!isMountedRef.current) return;
    setIsStatusLoading(true);
    try {
      const status = await shimService.getStatus();
      if (isMountedRef.current && seq === fetchSeqRef.current) {
        setShimStatus(status);
        setShimActionError(null);
      }
    } catch (err) {
      if (isMountedRef.current && seq === fetchSeqRef.current) {
        console.warn('Failed to fetch shim status:', err);
        setShimActionError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (isMountedRef.current && seq === fetchSeqRef.current) {
        setIsStatusLoading(false);
      }
    }
  }, []);

  React.useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      fetchSeqRef.current++;
    };
  }, []);
  React.useEffect(() => {
    if (isVisible) {
      void loadShimStatus();
    }
  }, [isVisible, loadShimStatus]);

  const handleInstallShim = async () => {
    setActiveAction('install');
    setShimActionError(null);
    try {
      await shimService.install();
      await loadShimStatus();
    } catch (err) {
      if (isMountedRef.current) {
        setShimActionError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (isMountedRef.current) {
        setActiveAction(null);
      }
    }
  };

  const handleUninstallShim = async () => {
    setActiveAction('uninstall');
    setShimActionError(null);
    try {
      await shimService.uninstall();
      await loadShimStatus();
    } catch (err) {
      if (isMountedRef.current) {
        setShimActionError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (isMountedRef.current) {
        setActiveAction(null);
      }
    }
  };

  const handleSetDefaultChannel = async (channel: ShimChannel) => {
    const previousChannel = shimStatus?.defaultChannel;
    setActiveAction('channel');
    setShimActionError(null);
    if (shimStatus) {
      setShimStatus({
        ...shimStatus,
        defaultChannel: channel,
      });
    }
    try {
      await shimService.setDefaultChannel(channel);
      await loadShimStatus();
    } catch (err) {
      if (isMountedRef.current) {
        if (previousChannel) {
          setShimStatus((current) =>
            current ? { ...current, defaultChannel: previousChannel } : current
          );
        }
        setShimActionError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (isMountedRef.current) {
        setActiveAction(null);
      }
    }
  };

  React.useEffect(() => {
    if (!isVisible && !isPrewarming) {
      return;
    }

    const markerPrefix = isPrewarming ? 'settings.prewarm.general' : 'settings.general';
    markSettingsPerf(`${markerPrefix}.commit`);
    const frameId = requestAnimationFrame(() => {
      markSettingsPerf(`${markerPrefix}.raf`);
    });

    return () => cancelAnimationFrame(frameId);
  }, [isPrewarming, isVisible]);

  const appLanguage = config.appLanguage || 'auto';
  const theme = config.theme || 'auto';
  const font = config.font || 'system';
  const minimizeToTrayOnExit = config.minimizeToTrayOnExit ?? true;
  const autoStart = config.autoStart ?? false;
  const silentStart = config.silentStart ?? false;
  const autoCheckUpdates = config.autoCheckUpdates ?? true;
  const logLevel = normalizeLogLevel(config.logLevel);

  return (
    <SettingsTabContainer id="settings-panel-general" ariaLabelledby="settings-tab-general">
      <SettingsPageHeader
        icon={<GeneralIcon width={28} height={28} />}
        title={t('settings.general')}
        description={t('settings.general_description')}
      />
      <SettingsSection title={t('settings.general_title')} icon={<Languages size={20} />}>
        <SettingsItem title={t('settings.language')} hint={t('settings.language_hint')}>
          <div style={{ width: '200px' }}>
            <Dropdown
              id="settings-language"
              value={appLanguage}
              onChange={(value) => updateConfig({ appLanguage: value as AppLanguagePreference })}
              options={APP_LANGUAGE_OPTIONS.map((option) => ({
                value: option.value,
                label: t(option.labelKey, { defaultValue: option.defaultLabel }),
              }))}
            />
          </div>
        </SettingsItem>

        <SettingsItem id="settings-theme" title={t('settings.theme')} layout="vertical">
          <div className="theme-selector-container">
            <button
              className={`theme-card ${theme === 'light' ? 'active' : ''}`}
              onClick={() => updateConfig({ theme: 'light' })}
              aria-label={t('settings.theme_light')}
              aria-pressed={theme === 'light'}
            >
              <div className="theme-preview light">
                <div className="theme-preview-window">
                  <div className="theme-preview-lines">
                    <div className="line short"></div>
                    <div className="line long"></div>
                  </div>
                  <div className="theme-preview-sidebar"></div>
                </div>
              </div>
              <span className="theme-label">{t('settings.theme_light')}</span>
            </button>

            <button
              className={`theme-card ${theme === 'dark' ? 'active' : ''}`}
              onClick={() => updateConfig({ theme: 'dark' })}
              aria-label={t('settings.theme_dark')}
              aria-pressed={theme === 'dark'}
            >
              <div className="theme-preview dark">
                <div className="theme-preview-window">
                  <div className="theme-preview-lines">
                    <div className="line short"></div>
                    <div className="line long"></div>
                  </div>
                  <div className="theme-preview-sidebar"></div>
                </div>
              </div>
              <span className="theme-label">{t('settings.theme_dark')}</span>
            </button>

            <button
              className={`theme-card ${theme === 'auto' ? 'active' : ''}`}
              onClick={() => updateConfig({ theme: 'auto' })}
              aria-label={t('common.auto')}
              aria-pressed={theme === 'auto'}
            >
              <div className="theme-preview auto">
                <div className="theme-preview-window">
                  <div className="theme-preview-lines">
                    <div className="line short"></div>
                    <div className="line long"></div>
                  </div>
                  <div className="theme-preview-sidebar"></div>
                </div>
              </div>
              <span className="theme-label">{t('common.auto')}</span>
            </button>
          </div>
        </SettingsItem>

        <SettingsItem title={t('settings.font')}>
          <div style={{ width: '240px' }}>
            <Dropdown
              id="settings-font"
              value={font}
              onChange={(value) => updateConfig({ font: value as FontValue })}
              options={[
                {
                  value: 'system',
                  label: t('settings.font_system'),
                  style: { fontFamily: 'inherit' },
                },
                { value: 'serif', label: t('settings.font_serif'), style: { fontFamily: 'serif' } },
                {
                  value: 'sans',
                  label: t('settings.font_sans'),
                  style: { fontFamily: 'sans-serif' },
                },
                {
                  value: 'mono',
                  label: t('settings.font_mono'),
                  style: { fontFamily: 'monospace' },
                },
                { value: 'arial', label: t('settings.font_arial'), style: { fontFamily: 'Arial' } },
                {
                  value: 'georgia',
                  label: t('settings.font_georgia'),
                  style: { fontFamily: 'Georgia' },
                },
              ]}
              style={{ fontFamily: getFontFamily(font) }}
            />
          </div>
        </SettingsItem>
      </SettingsSection>

      <SettingsSection>
        <SettingsItem title={t('settings.log_level')} hint={t('settings.log_level_hint')}>
          <div style={{ width: '180px' }}>
            <Dropdown
              id="settings-log-level"
              aria-label={t('settings.log_level')}
              value={logLevel}
              onChange={(value) => updateConfig({ logLevel: value as AppLogLevel })}
              options={APP_LOG_LEVELS.map((level) => ({
                value: level,
                label: t(`settings.log_level_${level}`),
              }))}
            />
          </div>
        </SettingsItem>

        <SettingsItem
          id="settings-auto-start"
          title={t('settings.auto_start')}
          hint={t('settings.auto_start_hint')}
        >
          <Switch
            checked={autoStart}
            onChange={(enabled) =>
              updateConfig({
                autoStart: enabled,
                ...(!enabled ? { silentStart: false } : {}),
              })
            }
          />
        </SettingsItem>

        <SettingsItem
          id="settings-tray-exit"
          title={t('settings.minimize_to_tray')}
          hint={t('settings.minimize_to_tray_hint')}
        >
          <Switch
            checked={minimizeToTrayOnExit}
            onChange={(enabled) =>
              updateConfig({
                minimizeToTrayOnExit: enabled,
                ...(!enabled ? { silentStart: false } : {}),
              })
            }
          />
        </SettingsItem>

        <SettingsItem
          id="settings-silent-start"
          title={t('settings.silent_start')}
          hint={
            !autoStart || !minimizeToTrayOnExit
              ? t('settings.silent_start_disabled_hint', {
                  defaultValue:
                    'Requires "Launch on startup" and "Minimize to tray on exit" to be enabled.',
                })
              : t('settings.silent_start_hint')
          }
        >
          <Switch
            checked={autoStart && minimizeToTrayOnExit && silentStart}
            disabled={!autoStart || !minimizeToTrayOnExit}
            onChange={(enabled) => updateConfig({ silentStart: enabled })}
          />
        </SettingsItem>

        <SettingsItem id="settings-auto-check-updates" title={t('settings.auto_check_updates')}>
          <Switch
            checked={autoCheckUpdates}
            onChange={(enabled) => updateConfig({ autoCheckUpdates: enabled })}
          />
        </SettingsItem>
      </SettingsSection>

      <SettingsSection
        title={t('settings.cli_tools.title')}
        description={t('settings.cli_tools.description')}
        icon={<Terminal size={20} />}
      >
        <SettingsItem
          title={t('settings.cli_tools.status_title')}
          hint={
            shimStatus === null
              ? ''
              : shimStatus.isConfigured
                ? t('settings.cli_tools.configured_hint')
                : t('settings.cli_tools.not_configured_hint')
          }
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
            <span
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '6px',
                fontSize: '13px',
                fontWeight: 500,
                color: shimStatus?.isConfigured
                  ? 'var(--color-success, #10b981)'
                  : 'var(--color-text-muted, #888)',
              }}
            >
              {shimStatus === null ? (
                <span style={{ opacity: 0.6 }}>...</span>
              ) : shimStatus.isConfigured ? (
                <>
                  <CheckCircle2 size={16} />
                  {t('settings.cli_tools.status_configured')}
                </>
              ) : (
                <>
                  <XCircle size={16} />
                  {t('settings.cli_tools.status_not_configured')}
                </>
              )}
            </span>
            {shimStatus === null && shimActionError ? (
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => void loadShimStatus()}
                disabled={isShimLoading}
              >
                {t('common.retry')}
              </button>
            ) : shimStatus?.isConfigured ? (
              <button
                type="button"
                className="btn btn-secondary"
                onClick={handleUninstallShim}
                disabled={isShimLoading || shimStatus === null}
              >
                {activeAction === 'uninstall'
                  ? t('settings.cli_tools.uninstalling')
                  : t('settings.cli_tools.uninstall_btn')}
              </button>
            ) : (
              <button
                type="button"
                className="btn btn-primary"
                onClick={handleInstallShim}
                disabled={isShimLoading || shimStatus === null}
              >
                {activeAction === 'install'
                  ? t('settings.cli_tools.installing')
                  : t('settings.cli_tools.install_btn')}
              </button>
            )}
          </div>
        </SettingsItem>

        {shimStatus && (
          <SettingsItem title={t('settings.cli_tools.bin_dir_title')}>
            <code
              style={{
                fontSize: '12px',
                padding: '4px 8px',
                background: 'var(--color-bg-secondary, rgba(0,0,0,0.05))',
                borderRadius: '4px',
                wordBreak: 'break-all',
              }}
            >
              {shimStatus.shimDir}
            </code>
          </SettingsItem>
        )}

        {shimStatus?.stableInstalled && shimStatus.nightlyInstalled && (
          <SettingsItem
            title={t('settings.cli_tools.default_channel_title')}
            hint={
              <div>
                <div>{t('settings.cli_tools.default_channel_hint')}</div>
                <div style={{ marginTop: '4px', fontSize: '12px', opacity: 0.8 }}>
                  {t('settings.cli_tools.coexistence_note')}
                </div>
              </div>
            }
          >
            <div
              role="radiogroup"
              aria-label={t('settings.cli_tools.default_channel_title')}
              style={{ display: 'flex', gap: '16px', alignItems: 'center' }}
            >
              <label
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '6px',
                  cursor: 'pointer',
                  fontSize: '13px',
                }}
              >
                <input
                  type="radio"
                  name="defaultChannel"
                  value="stable"
                  checked={shimStatus.defaultChannel === 'stable'}
                  onChange={() => void handleSetDefaultChannel('stable')}
                  disabled={isShimLoading}
                />
                {t('settings.cli_tools.channel_stable')}
              </label>
              <label
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '6px',
                  cursor: 'pointer',
                  fontSize: '13px',
                }}
              >
                <input
                  type="radio"
                  name="defaultChannel"
                  value="nightly"
                  checked={shimStatus.defaultChannel === 'nightly'}
                  onChange={() => void handleSetDefaultChannel('nightly')}
                  disabled={isShimLoading}
                />
                {t('settings.cli_tools.channel_nightly')}
              </label>
            </div>
          </SettingsItem>
        )}

        {shimActionError && (
          <div
            role="alert"
            style={{
              padding: '8px 12px',
              color: 'var(--color-danger, #ef4444)',
              fontSize: '12px',
              borderRadius: '4px',
              background: 'rgba(239, 68, 68, 0.1)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              gap: '8px',
            }}
          >
            <span>{shimActionError}</span>
            {shimStatus !== null && (
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => void loadShimStatus()}
                disabled={isShimLoading}
                style={{ fontSize: '11px', padding: '2px 8px' }}
              >
                {t('common.retry')}
              </button>
            )}
          </div>
        )}
      </SettingsSection>
      <SettingsSection
        title={t('settings.diagnostics.title', { defaultValue: 'Model & Environment Diagnostics' })}
        description={t('settings.diagnostics.entry_description', {
          defaultValue:
            'Open a dedicated diagnostics page for the local transcription path, runtime readiness, and packaged environment checks.',
        })}
        icon={<Stethoscope size={20} />}
      >
        <SettingsItem
          id="settings-diagnostics"
          title={t('settings.diagnostics.entry_title', { defaultValue: 'Diagnostics' })}
          hint={t('settings.diagnostics.entry_hint', {
            defaultValue:
              'Review the current local setup and jump straight to the clearest fix when something is off.',
          })}
        >
          <button
            type="button"
            className="btn btn-secondary"
            onClick={onOpenDiagnostics}
            disabled={!onOpenDiagnostics}
          >
            {t('settings.diagnostics.open_button', { defaultValue: 'Open Diagnostics' })}
          </button>
        </SettingsItem>
      </SettingsSection>
    </SettingsTabContainer>
  );
}

export default SettingsGeneralTab;
