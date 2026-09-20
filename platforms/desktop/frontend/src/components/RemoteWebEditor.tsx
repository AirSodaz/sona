import { Check, Globe, Key, Loader2, Monitor, Moon, Server, Sun, Trash2 } from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { APP_LANGUAGE_OPTIONS, resolveAppLanguagePreference } from '../constants/appLanguages';
import {
  type ApiServerInfo,
  type ApiServerModelInfo,
  apiServerClient,
} from '../services/apiServerClient';
import { ONLINE_ASR_PROVIDER_DEFINITIONS } from '../services/onlineAsrProviders';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { PRESET_MODELS_MAP } from '../types/modelCatalog';
import { exportToMarkdown, exportToSrt, exportToTxt, exportToVtt } from '../utils/webExport';
import { AudioPlayer } from './AudioPlayer';
import { Dropdown, type DropdownOption } from './Dropdown';
import { ErrorDialog } from './ErrorDialog';
import { GlobalDialog } from './GlobalDialog';
import { CloseIcon, DownloadIcon, FileTextIcon, UploadIcon } from './Icons';
import { TranscriptEditor } from './transcript/TranscriptEditor';

function isAsrModel(model: string | ApiServerModelInfo): boolean {
  const id = typeof model === 'string' ? model : model.id;
  const preset = PRESET_MODELS_MAP.get(id);
  if (preset) {
    if (
      preset.type === 'vad' ||
      preset.type === 'punctuation' ||
      preset.type === 'speaker-segmentation' ||
      preset.type === 'speaker-embedding' ||
      preset.type === 'alignment'
    ) {
      return false;
    }
    return Boolean(preset.modes && preset.modes.length > 0);
  }
  if (typeof model === 'object' && model !== null) {
    const raw = model as unknown as Record<string, unknown>;
    const type = (raw.type || raw.modelType) as string | undefined;
    if (
      type === 'vad' ||
      type === 'punctuation' ||
      type === 'speaker-segmentation' ||
      type === 'speaker-embedding' ||
      type === 'alignment'
    ) {
      return false;
    }
  }
  const lowerId = id.toLowerCase();
  if (
    lowerId.includes('vad') ||
    lowerId.includes('punctuation') ||
    lowerId.includes('punct') ||
    lowerId.includes('segmentation') ||
    lowerId.includes('embedding')
  ) {
    return false;
  }
  return true;
}

function getModelLabel(model: string | ApiServerModelInfo): string {
  const id = typeof model === 'string' ? model : model.id;
  const preset = PRESET_MODELS_MAP.get(id);

  if (typeof model === 'object' && model !== null && model.name) {
    if (preset?.versionLabel && !model.name.includes(preset.versionLabel)) {
      return `${model.name} (${preset.versionLabel})`;
    }
    return model.name;
  }

  if (preset) {
    return preset.versionLabel ? `${preset.name} (${preset.versionLabel})` : preset.name;
  }

  return id;
}
export function RemoteWebEditor(): React.JSX.Element {
  const { t, i18n } = useTranslation();
  // Stores
  const segments = useTranscriptSessionStore((state) => state.segments);
  const title = useTranscriptSessionStore((state) => state.title);
  const setTitle = useTranscriptSessionStore((state) => state.setTitle);
  const setSegments = useTranscriptSessionStore((state) => state.setSegments);
  const clearActiveTranscriptSession = useTranscriptSessionStore(
    (state) => state.clearActiveTranscriptSession
  );
  const audioUrl = useTranscriptPlaybackStore((state) => state.audioUrl);
  const setAudioUrl = useTranscriptPlaybackStore((state) => state.setAudioUrl);

  // Connection & Auth State
  const [serverUrl, setServerUrl] = useState<string>(() => apiServerClient.getBaseUrl());
  const [apiKey, setApiKey] = useState<string>(() => apiServerClient.getApiKey());
  const [isConnected, setIsConnected] = useState<boolean>(false);
  const [isConnecting, setIsConnecting] = useState<boolean>(false);
  const [serverInfo, setServerInfo] = useState<ApiServerInfo | null>(null);

  // Modals
  const [showServerModal, setShowServerModal] = useState<boolean>(false);
  const [tempServerUrl, setTempServerUrl] = useState<string>(serverUrl);
  const [showApiKeyModal, setShowApiKeyModal] = useState<boolean>(false);
  const [tempApiKey, setTempApiKey] = useState<string>(apiKey);
  const [showExportMenu, setShowExportMenu] = useState<boolean>(false);

  // Theme State
  const [themePreference, setThemePreference] = useState<'auto' | 'light' | 'dark'>(() => {
    try {
      const saved = localStorage.getItem('sona_web_theme');
      if (saved === 'light' || saved === 'dark' || saved === 'auto') {
        return saved;
      }
    } catch {
      // ignore
    }
    return 'auto';
  });
  const [showThemeMenu, setShowThemeMenu] = useState<boolean>(false);
  const themeMenuRef = useRef<HTMLDivElement>(null);

  // Language State
  const [languagePreference, setLanguagePreference] = useState<string>(() => {
    try {
      return localStorage.getItem('sona_web_language') || 'auto';
    } catch {
      return 'auto';
    }
  });
  const [showLanguageMenu, setShowLanguageMenu] = useState<boolean>(false);
  const languageMenuRef = useRef<HTMLDivElement>(null);

  // Form State
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [selectedModel, setSelectedModel] = useState<string>('');
  const [selectedLanguage, setSelectedLanguage] = useState<string>('auto');
  const [isDragOver, setIsDragOver] = useState<boolean>(false);

  // Transcription Job State
  const [isTranscribing, setIsTranscribing] = useState<boolean>(false);
  const [transcribeProgress, setTranscribeProgress] = useState<string | null>(null);
  const [transcribeError, setTranscribeError] = useState<string | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const pollTimerRef = useRef<number | null>(null);
  const exportMenuRef = useRef<HTMLDivElement>(null);
  const currentBlobUrlRef = useRef<string | null>(null);
  const tRef = useRef(t);
  useEffect(() => {
    tRef.current = t;
  }, [t]);

  const connectToServer = useCallback(async () => {
    setIsConnecting(true);
    setTranscribeError(null);
    try {
      await apiServerClient.checkHealth();
      const info = await apiServerClient.getInfo();
      setServerInfo(info);
      setIsConnected(true);

      let defaultModel = '';
      if (info.models?.length > 0) {
        const first = info.models.find(isAsrModel);
        if (first) {
          defaultModel = typeof first === 'string' ? first : first.id || '';
        }
      }
      if (!defaultModel && info.onlineAsrProviders?.length) {
        const configuredBatch = info.onlineAsrProviders.find(
          (p) => p.configured && p.supportsBatch
        );
        if (configuredBatch) {
          defaultModel = configuredBatch.id;
        }
      }
      if (defaultModel) {
        setSelectedModel((prev) => prev || defaultModel);
      }
    } catch (err: unknown) {
      setIsConnected(false);
      setServerInfo(null);
      const errMsg = err instanceof Error ? err.message : String(err);
      if (errMsg.includes('401')) {
        setTranscribeError(
          tRef.current('web.auth_required_error', {
            defaultValue:
              'API server requires authentication (401). Please configure your API Key.',
          })
        );
      }
    } finally {
      setIsConnecting(false);
    }
  }, []);

  useEffect(() => {
    connectToServer();
  }, [connectToServer]);

  // Clean up timer and local blob URL
  useEffect(() => {
    return () => {
      clearInterval(pollTimerRef.current as number);
      if (currentBlobUrlRef.current) {
        URL.revokeObjectURL(currentBlobUrlRef.current);
        currentBlobUrlRef.current = null;
      }
    };
  }, []);
  // Close popups on click outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (exportMenuRef.current && !exportMenuRef.current.contains(target)) {
        setShowExportMenu(false);
      }
      if (languageMenuRef.current && !languageMenuRef.current.contains(target)) {
        setShowLanguageMenu(false);
      }
      if (themeMenuRef.current && !themeMenuRef.current.contains(target)) {
        setShowThemeMenu(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  // Close popups and modals on Escape
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setShowExportMenu(false);
        setShowLanguageMenu(false);
        setShowThemeMenu(false);
        setShowServerModal(false);
        setShowApiKeyModal(false);
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Synchronize theme preference with document element
  useEffect(() => {
    const root = document.documentElement;

    const applyTheme = (theme: 'auto' | 'light' | 'dark') => {
      let resolved: 'light' | 'dark' = 'light';
      if (theme === 'auto') {
        if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
          resolved = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
        } else {
          resolved = 'light';
        }
      } else {
        resolved = theme;
      }
      root.setAttribute('data-theme', resolved);
    };

    applyTheme(themePreference);

    if (
      themePreference === 'auto' &&
      typeof window !== 'undefined' &&
      typeof window.matchMedia === 'function'
    ) {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const handleChange = (e: MediaQueryListEvent) => {
        root.setAttribute('data-theme', e.matches ? 'dark' : 'light');
      };
      mediaQuery.addEventListener?.('change', handleChange);
      return () => {
        mediaQuery.removeEventListener?.('change', handleChange);
      };
    }
  }, [themePreference]);

  const handleSelectTheme = (theme: 'auto' | 'light' | 'dark') => {
    setThemePreference(theme);
    try {
      localStorage.setItem('sona_web_theme', theme);
    } catch {
      // ignore
    }
    setShowThemeMenu(false);
  };

  // Synchronize language preference with i18n
  useEffect(() => {
    const resolved = resolveAppLanguagePreference(languagePreference, navigator.language);
    void i18n.changeLanguage(resolved);
  }, [languagePreference, i18n]);

  const handleSelectLanguage = (lang: string) => {
    setLanguagePreference(lang);
    try {
      localStorage.setItem('sona_web_language', lang);
    } catch {
      // ignore
    }
    const resolved = resolveAppLanguagePreference(lang, navigator.language);
    void i18n.changeLanguage(resolved);
    setShowLanguageMenu(false);
  };

  const currentLanguageLabel = useMemo(() => {
    if (languagePreference === 'auto') {
      return t('web.language_auto', { defaultValue: 'System' });
    }
    const found = APP_LANGUAGE_OPTIONS.find((o) => o.value === languagePreference);
    return found ? found.defaultLabel : languagePreference;
  }, [languagePreference, t]);

  const currentThemeLabel = useMemo(() => {
    if (themePreference === 'dark') {
      return t('web.theme_dark', { defaultValue: 'Dark' });
    }
    if (themePreference === 'light') {
      return t('web.theme_light', { defaultValue: 'Light' });
    }
    return t('web.theme_auto', { defaultValue: 'System' });
  }, [themePreference, t]);

  const modelOptions: DropdownOption[] = useMemo(() => {
    const options: DropdownOption[] = [];
    if (serverInfo?.models) {
      for (const m of serverInfo.models) {
        if (!isAsrModel(m)) {
          continue;
        }
        const modelId = typeof m === 'string' ? m : m.id;
        const label = getModelLabel(m);
        options.push({
          value: modelId,
          label,
          ariaLabel: label,
        });
      }
    }
    if (serverInfo?.onlineAsrProviders) {
      for (const p of serverInfo.onlineAsrProviders) {
        if (p.configured && p.supportsBatch) {
          const providerDef = ONLINE_ASR_PROVIDER_DEFINITIONS.find((def) => def.id === p.id);
          const providerName = providerDef
            ? t(providerDef.optionLabelKey, { defaultValue: providerDef.optionDefaultLabel })
            : p.id;
          const onlineBadge = t('web.online_badge', { defaultValue: '在线' });
          const label = `${providerName} (${onlineBadge})`;
          options.push({
            value: p.id,
            label,
            ariaLabel: label,
          });
        }
      }
    }
    return options;
  }, [serverInfo, t]);

  useEffect(() => {
    if (modelOptions.length > 0) {
      const isValid = modelOptions.some((opt) => opt.value === selectedModel);
      if (!isValid) {
        setSelectedModel(modelOptions[0].value);
      }
    }
  }, [selectedModel, modelOptions]);

  const languageOptions: DropdownOption[] = useMemo(
    () => [
      { value: 'auto', label: t('web.lang_auto', { defaultValue: 'Auto Detect' }) },
      { value: 'zh', label: t('web.lang_zh', { defaultValue: 'Chinese' }) },
      { value: 'en', label: t('web.lang_en', { defaultValue: 'English' }) },
      { value: 'ja', label: t('web.lang_ja', { defaultValue: 'Japanese' }) },
      { value: 'ko', label: t('web.lang_ko', { defaultValue: 'Korean' }) },
      { value: 'yue', label: t('web.lang_yue', { defaultValue: 'Cantonese' }) },
    ],
    [t]
  );

  // Save server URL
  const handleSaveServerUrl = () => {
    const trimmed = tempServerUrl.trim();
    if (trimmed) {
      apiServerClient.setBaseUrl(trimmed);
      setServerUrl(trimmed);
      setShowServerModal(false);
      connectToServer();
    }
  };

  // Save API Key
  const handleSaveApiKey = () => {
    const trimmed = tempApiKey.trim();
    apiServerClient.setApiKey(trimmed);
    setApiKey(trimmed);
    setShowApiKeyModal(false);
    connectToServer();
  };

  // Handle file selection
  const handleFileSelect = (file: File) => {
    setSelectedFile(file);
    setTitle(file.name.replace(/\.[^/.]+$/, ''));
    setTranscribeError(null);
    if (currentBlobUrlRef.current) {
      URL.revokeObjectURL(currentBlobUrlRef.current);
    }
    // Create local object URL for preview audio playback immediately
    const localAudioUrl = URL.createObjectURL(file);
    currentBlobUrlRef.current = localAudioUrl;
    setAudioUrl(localAudioUrl);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
    if (e.dataTransfer.files?.[0]) {
      handleFileSelect(e.dataTransfer.files[0]);
    }
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files?.[0]) {
      handleFileSelect(e.target.files[0]);
    }
  };

  // Start transcription
  const handleStartTranscribe = async () => {
    if (!selectedFile) {
      setTranscribeError(t('batch.select_file', { defaultValue: 'Please select an audio file' }));
      return;
    }
    if (!selectedModel) {
      setTranscribeError(t('web.select_model', { defaultValue: 'Please select an ASR model' }));
      return;
    }

    setIsTranscribing(true);
    setTranscribeProgress(
      t('batch.transcribing', {
        defaultValue: 'Uploading audio and creating transcription task...',
      })
    );
    setTranscribeError(null);

    try {
      const jobId = await apiServerClient.transcribe(selectedFile, {
        modelId: selectedModel,
        language: selectedLanguage === 'auto' ? undefined : selectedLanguage,
      });

      setTranscribeProgress(
        t('web.transcribe_pending', { defaultValue: 'Queued, waiting to process...' })
      );

      clearInterval(pollTimerRef.current as number);

      pollTimerRef.current = window.setInterval(async () => {
        try {
          const status = await apiServerClient.getJobStatus(jobId);
          if (status === 'Pending') {
            setTranscribeProgress(
              t('web.transcribe_pending', { defaultValue: 'Queued, waiting to process...' })
            );
          } else if (status === 'Processing') {
            setTranscribeProgress(
              t('web.transcribe_processing', {
                defaultValue: 'Transcribing in progress, please wait...',
              })
            );
          } else if (typeof status === 'object' && 'Completed' in status) {
            clearInterval(pollTimerRef.current as number);
            pollTimerRef.current = null;
            setIsTranscribing(false);
            setTranscribeProgress(null);
            setSegments(status.Completed);

            // Only update audio url to server endpoint if local file blob is not available
            if (!currentBlobUrlRef.current) {
              const serverAudioUrl = apiServerClient.getAudioUrl(jobId);
              setAudioUrl(serverAudioUrl);
            }
          } else if (typeof status === 'object' && 'Failed' in status) {
            clearInterval(pollTimerRef.current as number);
            pollTimerRef.current = null;
            setIsTranscribing(false);
            setTranscribeProgress(null);
            setTranscribeError(
              t('web.transcribe_failed', {
                error: status.Failed,
                defaultValue: `Transcription failed: ${status.Failed}`,
              })
            );
          }
        } catch (pollErr: unknown) {
          clearInterval(pollTimerRef.current as number);
          pollTimerRef.current = null;
          setIsTranscribing(false);
          setTranscribeProgress(null);
          const errMsg = pollErr instanceof Error ? pollErr.message : String(pollErr);
          setTranscribeError(
            t('web.query_status_failed', {
              error: errMsg,
              defaultValue: `Failed to query status: ${errMsg}`,
            })
          );
        }
      }, 1500);
    } catch (err: unknown) {
      setIsTranscribing(false);
      setTranscribeProgress(null);
      const errMsg = err instanceof Error ? err.message : String(err);
      setTranscribeError(
        t('web.start_failed', {
          error: errMsg,
          defaultValue: `Failed to start transcription: ${errMsg}`,
        })
      );
    }
  };

  // Export handling
  const downloadFile = (content: string, filename: string, mime: string) => {
    const blob = new Blob([content], { type: mime });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  const handleExport = (format: 'srt' | 'vtt' | 'txt' | 'json' | 'md') => {
    const baseName = title || 'transcript';
    let content = '';
    const ext = format;
    let mime = 'text/plain;charset=utf-8';

    switch (format) {
      case 'srt':
        content = exportToSrt(segments);
        break;
      case 'vtt':
        content = exportToVtt(segments);
        mime = 'text/vtt;charset=utf-8';
        break;
      case 'txt':
        content = exportToTxt(segments);
        break;
      case 'json':
        content = JSON.stringify(segments, null, 2);
        mime = 'application/json;charset=utf-8';
        break;
      case 'md':
        content = exportToMarkdown(segments);
        mime = 'text/markdown;charset=utf-8';
        break;
    }
    downloadFile(content, `${baseName}.${ext}`, mime);
    setShowExportMenu(false);
  };

  const handleClearSession = () => {
    if (
      segments.length > 0 &&
      !confirm(
        t('web.clear_confirm', {
          defaultValue: 'Are you sure you want to clear the current transcript?',
        })
      )
    ) {
      return;
    }
    clearActiveTranscriptSession();
    setSelectedFile(null);
    if (currentBlobUrlRef.current) {
      URL.revokeObjectURL(currentBlobUrlRef.current);
      currentBlobUrlRef.current = null;
    }
    setAudioUrl(null);
    if (fileInputRef.current) {
      fileInputRef.current.value = '';
    }
  };

  const displayTitle =
    title ||
    selectedFile?.name ||
    t('web.untitled_transcript', { defaultValue: 'Untitled Transcript' });

  return (
    <div className="app">
      {/* App Header */}
      <header className="app-header">
        <div className="app-logo">
          <h1>Sona</h1>
          <span
            style={{
              fontSize: '11px',
              fontWeight: 500,
              padding: '2px 6px',
              borderRadius: 'var(--radius-sm, 4px)',
              background: 'var(--color-bg-tertiary)',
              color: 'var(--color-text-secondary)',
              marginLeft: '6px',
            }}
          >
            {t('web.title_badge', { defaultValue: 'Web' })}
          </span>
        </div>

        <div
          className="header-actions"
          style={{ display: 'flex', alignItems: 'center', gap: '8px' }}
        >
          {/* Server Connection Pill */}
          <button
            type="button"
            className="web-header-pill"
            onClick={() => {
              setTempServerUrl(serverUrl);
              setShowServerModal(true);
            }}
            title={t('web.server_url_tooltip', {
              defaultValue: 'Click to change server host address',
            })}
          >
            <span
              className={`web-status-dot ${
                isConnecting ? 'connecting' : isConnected ? 'connected' : 'disconnected'
              }`}
            />
            <Server size={13} />
            <span>{serverUrl.replace(/^https?:\/\//, '')}</span>
          </button>

          {/* API Key Pill */}
          <button
            type="button"
            className="web-header-pill"
            onClick={() => {
              setTempApiKey(apiKey);
              setShowApiKeyModal(true);
            }}
            title={t('web.api_key_tooltip', { defaultValue: 'Configure API Key auth token' })}
          >
            <Key size={13} style={{ color: apiKey ? 'var(--color-success)' : undefined }} />
            <span>
              {apiKey
                ? t('web.api_key_set', { defaultValue: 'API Key: Set' })
                : t('web.api_key_not_set', { defaultValue: 'API Key: Not Set' })}
            </span>
          </button>

          {/* Language Switcher Pill & Dropdown */}
          <div className="web-header-dropdown-wrap" ref={languageMenuRef}>
            <button
              type="button"
              className={`web-header-pill ${showLanguageMenu ? 'active' : ''}`}
              onClick={() => {
                setShowLanguageMenu((prev) => !prev);
                setShowThemeMenu(false);
                setShowExportMenu(false);
              }}
              title={t('web.language_tooltip', { defaultValue: 'Switch language' })}
              aria-label={t('web.language_tooltip', { defaultValue: 'Switch language' })}
              aria-expanded={showLanguageMenu}
            >
              <Globe size={13} />
              <span>{currentLanguageLabel}</span>
            </button>
            {showLanguageMenu && (
              <div className="web-header-dropdown-menu">
                {APP_LANGUAGE_OPTIONS.map((opt) => (
                  <button
                    key={opt.value}
                    type="button"
                    className={`web-dropdown-item ${
                      languagePreference === opt.value ? 'selected' : ''
                    }`}
                    onClick={() => handleSelectLanguage(opt.value)}
                  >
                    <span>
                      {opt.value === 'auto'
                        ? t('web.language_auto', { defaultValue: 'System' })
                        : opt.defaultLabel}
                    </span>
                    {languagePreference === opt.value && <Check size={13} />}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Theme Switcher Pill & Dropdown */}
          <div className="web-header-dropdown-wrap" ref={themeMenuRef}>
            <button
              type="button"
              className={`web-header-pill ${showThemeMenu ? 'active' : ''}`}
              onClick={() => {
                setShowThemeMenu((prev) => !prev);
                setShowLanguageMenu(false);
                setShowExportMenu(false);
              }}
              title={t('web.theme_tooltip', { defaultValue: 'Toggle theme' })}
              aria-label={t('web.theme_tooltip', { defaultValue: 'Toggle theme' })}
              aria-expanded={showThemeMenu}
            >
              {themePreference === 'dark' ? (
                <Moon size={13} />
              ) : themePreference === 'light' ? (
                <Sun size={13} />
              ) : (
                <Monitor size={13} />
              )}
              <span>{currentThemeLabel}</span>
            </button>
            {showThemeMenu && (
              <div className="web-header-dropdown-menu">
                <button
                  type="button"
                  className={`web-dropdown-item ${themePreference === 'auto' ? 'selected' : ''}`}
                  onClick={() => handleSelectTheme('auto')}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <Monitor size={13} />
                    <span>{t('web.theme_auto', { defaultValue: 'System' })}</span>
                  </div>
                  {themePreference === 'auto' && <Check size={13} />}
                </button>
                <button
                  type="button"
                  className={`web-dropdown-item ${themePreference === 'light' ? 'selected' : ''}`}
                  onClick={() => handleSelectTheme('light')}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <Sun size={13} />
                    <span>{t('web.theme_light', { defaultValue: 'Light' })}</span>
                  </div>
                  {themePreference === 'light' && <Check size={13} />}
                </button>
                <button
                  type="button"
                  className={`web-dropdown-item ${themePreference === 'dark' ? 'selected' : ''}`}
                  onClick={() => handleSelectTheme('dark')}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <Moon size={13} />
                    <span>{t('web.theme_dark', { defaultValue: 'Dark' })}</span>
                  </div>
                  {themePreference === 'dark' && <Check size={13} />}
                </button>
              </div>
            )}
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main id="main-content" className="app-main">
        <div className="panel-container">
          {/* Left Panel: Transcribe / Input */}
          <div
            className="panel panel-left"
            style={{ width: '360px', minWidth: '320px', maxWidth: '420px', flex: '0 0 360px' }}
          >
            <div className="panel-header">
              <h2>{t('panel.batch_import', { defaultValue: 'Transcribe' })}</h2>
            </div>

            <div
              className="panel-content"
              style={{
                padding: '16px',
                overflowY: 'auto',
                display: 'flex',
                flexDirection: 'column',
                gap: '16px',
              }}
            >
              {/* Dropzone reusing desktop's .drop-zone */}
              <div
                className={`drop-zone drop-zone-wrapper ${isDragOver ? 'drag-over' : ''}`}
                style={{ minHeight: '160px', cursor: 'pointer', padding: '24px 16px' }}
                onClick={() => fileInputRef.current?.click()}
                onDrop={handleDrop}
                onDragOver={handleDragOver}
                onDragLeave={handleDragLeave}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    fileInputRef.current?.click();
                  }
                }}
              >
                <div className="drop-zone-icon">
                  <UploadIcon />
                </div>

                <div className="drop-zone-text">
                  <h3 style={{ fontSize: '14px', margin: '0 0 4px 0' }}>
                    {selectedFile
                      ? selectedFile.name
                      : t('batch.drop_title', {
                          defaultValue: 'Drop audio here or click to upload',
                        })}
                  </h3>
                  <p style={{ fontSize: '12px', margin: 0, color: 'var(--color-text-secondary)' }}>
                    {selectedFile
                      ? `${(selectedFile.size / (1024 * 1024)).toFixed(2)} MB`
                      : t('batch.drop_desc', {
                          defaultValue:
                            'Supports MP3, WAV, M4A, FLAC, OGG and other common formats',
                        })}
                  </p>
                </div>

                <div
                  className="btn btn-primary btn-sm"
                  style={{ marginTop: '8px', pointerEvents: 'none' }}
                  aria-hidden="true"
                >
                  {selectedFile
                    ? t('web.change_file', { defaultValue: 'Change File' })
                    : t('batch.select_file', { defaultValue: 'Select Audio File' })}
                </div>
              </div>

              <input
                ref={fileInputRef}
                type="file"
                accept="audio/*,video/*"
                style={{ display: 'none' }}
                onChange={handleFileChange}
              />

              {/* Form options */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                <div className="form-group">
                  <label className="form-label" style={{ fontSize: '12px', marginBottom: '6px' }}>
                    {t('settings.asr.model', { defaultValue: 'ASR Model' })}
                  </label>
                  <Dropdown
                    options={modelOptions}
                    value={selectedModel}
                    onChange={setSelectedModel}
                    placeholder={
                      !isConnected
                        ? t('web.connect_server_first', {
                            defaultValue: 'Please connect to server first',
                          })
                        : modelOptions.length === 0
                          ? t('web.no_models_available', { defaultValue: 'No models available' })
                          : t('web.select_model', { defaultValue: 'Select model...' })
                    }
                    disabled={isTranscribing || modelOptions.length === 0}
                  />
                  {isConnected && modelOptions.length === 0 && (
                    <div
                      style={{
                        fontSize: '11px',
                        color: 'var(--color-warning, #f59e0b)',
                        marginTop: '4px',
                      }}
                    >
                      {t('web.no_models_warning', {
                        defaultValue:
                          'No installed local model or online ASR found. Please download models in the desktop client.',
                      })}
                    </div>
                  )}
                </div>

                <div className="form-group">
                  <label className="form-label" style={{ fontSize: '12px', marginBottom: '6px' }}>
                    {t('settings.asr.language', { defaultValue: 'Audio Language' })}
                  </label>
                  <Dropdown
                    options={languageOptions}
                    value={selectedLanguage}
                    onChange={setSelectedLanguage}
                    disabled={isTranscribing}
                  />
                </div>
                {/* Submit button */}
                <button
                  type="button"
                  className="btn btn-primary"
                  style={{ width: '100%', height: '36px', marginTop: '4px' }}
                  disabled={!selectedFile || !selectedModel || isTranscribing || !isConnected}
                  onClick={handleStartTranscribe}
                >
                  {isTranscribing ? (
                    <>
                      <Loader2 size={15} className="animate-spin" />
                      <span>{t('web.transcribing', { defaultValue: 'Transcribing...' })}</span>
                    </>
                  ) : (
                    <>
                      <UploadIcon />
                      <span>
                        {t('web.start_transcribe', { defaultValue: 'Start Transcription' })}
                      </span>
                    </>
                  )}
                </button>

                {/* Progress message */}
                {transcribeProgress && (
                  <div
                    style={{
                      padding: '8px 12px',
                      background: 'var(--color-bg-secondary)',
                      borderRadius: 'var(--radius-sm, 6px)',
                      fontSize: '12px',
                      color: 'var(--color-text-secondary)',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px',
                    }}
                  >
                    <Loader2 size={13} className="animate-spin" />
                    <span>{transcribeProgress}</span>
                  </div>
                )}

                {/* Error message */}
                {transcribeError && (
                  <div
                    style={{
                      padding: '8px 12px',
                      background: 'var(--color-error-subtle, rgba(239, 68, 68, 0.1))',
                      border: '1px solid var(--color-error)',
                      borderRadius: 'var(--radius-sm, 6px)',
                      fontSize: '12px',
                      color: 'var(--color-error)',
                    }}
                  >
                    {transcribeError}
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Right Panel: Editor & Audio Player */}
          <div
            className="panel panel-right"
            style={{
              flex: 1,
              display: 'flex',
              flexDirection: 'column',
              height: '100%',
              overflow: 'hidden',
            }}
          >
            <div className="projects-detail-header">
              <div className="projects-detail-header-primary">
                <FileTextIcon />
                <h4
                  style={{
                    margin: 0,
                    whiteSpace: 'nowrap',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                  }}
                  title={displayTitle}
                >
                  {displayTitle}
                </h4>
                {segments.length > 0 && (
                  <span
                    style={{
                      fontSize: '12px',
                      color: 'var(--color-text-secondary)',
                      marginLeft: '8px',
                    }}
                  >
                    {t('web.segment_count', {
                      count: segments.length,
                      defaultValue: `${segments.length} segments`,
                    })}
                  </span>
                )}
              </div>

              <div className="projects-detail-header-actions">
                {segments.length > 0 && (
                  <>
                    {/* Export dropdown */}
                    <div className="export-menu" ref={exportMenuRef}>
                      <button
                        type="button"
                        className="btn btn-secondary btn-sm"
                        onClick={() => setShowExportMenu(!showExportMenu)}
                        style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}
                      >
                        <DownloadIcon />
                        <span>{t('export.button', { defaultValue: 'Export' })}</span>
                      </button>

                      {showExportMenu && (
                        <div className="export-dropdown">
                          <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={() => handleExport('srt')}
                          >
                            <span>{t('web.export_srt', { defaultValue: 'SubRip (.srt)' })}</span>
                          </button>
                          <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={() => handleExport('vtt')}
                          >
                            <span>{t('web.export_vtt', { defaultValue: 'WebVTT (.vtt)' })}</span>
                          </button>
                          <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={() => handleExport('txt')}
                          >
                            <span>
                              {t('web.export_txt', { defaultValue: 'Plain Text (.txt)' })}
                            </span>
                          </button>
                          <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={() => handleExport('md')}
                          >
                            <span>{t('web.export_md', { defaultValue: 'Markdown (.md)' })}</span>
                          </button>
                          <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={() => handleExport('json')}
                          >
                            <span>{t('web.export_json', { defaultValue: 'JSON (.json)' })}</span>
                          </button>
                        </div>
                      )}
                    </div>

                    {/* Clear session */}
                    <button
                      type="button"
                      className="btn btn-icon btn-sm"
                      onClick={handleClearSession}
                      title={t('web.clear_transcript', {
                        defaultValue: 'Clear current transcript',
                      })}
                    >
                      <Trash2 size={15} />
                    </button>
                  </>
                )}
              </div>
            </div>

            <div className="panel-content" style={{ flex: 1, overflow: 'hidden' }}>
              {segments.length > 0 ? (
                <TranscriptEditor />
              ) : (
                <div
                  style={{
                    height: '100%',
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    justifyContent: 'center',
                    color: 'var(--color-text-muted)',
                    gap: '12px',
                  }}
                >
                  <FileTextIcon style={{ width: '48px', height: '48px', opacity: 0.3 }} />
                  <p style={{ margin: 0, fontSize: '13px' }}>
                    {t('web.empty_tip', {
                      defaultValue:
                        'Select an audio file on the left and click "Start Transcription" to generate transcript',
                    })}
                  </p>
                </div>
              )}
            </div>

            {audioUrl && <AudioPlayer />}
          </div>
        </div>
      </main>

      {/* Server URL Modal */}
      {showServerModal && (
        <div className="web-modal-backdrop" onClick={() => setShowServerModal(false)}>
          <div className="web-modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="web-modal-header">
              <h3>{t('web.server_host_title', { defaultValue: 'Server Host Address' })}</h3>
              <button
                type="button"
                className="btn btn-icon btn-sm"
                onClick={() => setShowServerModal(false)}
              >
                <CloseIcon />
              </button>
            </div>
            <div className="web-modal-body">
              <p style={{ fontSize: '12px', color: 'var(--color-text-secondary)', margin: 0 }}>
                {t('web.server_host_desc', {
                  defaultValue:
                    'Specify the host address and port where desktop Sona is running (default port 14200).',
                })}
              </p>
              <input
                type="text"
                className="input-text"
                value={tempServerUrl}
                onChange={(e) => setTempServerUrl(e.target.value)}
                placeholder="http://192.168.1.100:14200"
                autoFocus
              />
            </div>
            <div className="web-modal-footer">
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setShowServerModal(false)}
              >
                {t('common.cancel', { defaultValue: 'Cancel' })}
              </button>
              <button
                type="button"
                className="btn btn-primary btn-sm"
                onClick={handleSaveServerUrl}
              >
                {t('web.save_and_connect', { defaultValue: 'Save & Connect' })}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* API Key Modal */}
      {showApiKeyModal && (
        <div className="web-modal-backdrop" onClick={() => setShowApiKeyModal(false)}>
          <div className="web-modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="web-modal-header">
              <h3>{t('web.api_key_title', { defaultValue: 'API Key Auth Token' })}</h3>
              <button
                type="button"
                className="btn btn-icon btn-sm"
                onClick={() => setShowApiKeyModal(false)}
              >
                <CloseIcon />
              </button>
            </div>
            <div className="web-modal-body">
              <p style={{ fontSize: '12px', color: 'var(--color-text-secondary)', margin: 0 }}>
                {t('web.api_key_desc', {
                  defaultValue:
                    'If the desktop client has enabled API Key authentication, enter the matching key here. It will be stored locally and attached to transcription requests and audio streams.',
                })}
              </p>
              <input
                type="password"
                className="input-text"
                value={tempApiKey}
                onChange={(e) => setTempApiKey(e.target.value)}
                placeholder={t('web.api_key_placeholder', {
                  defaultValue: 'Enter API Key, leave blank for no auth',
                })}
                autoFocus
              />
            </div>
            <div className="web-modal-footer">
              {apiKey && (
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  style={{ marginRight: 'auto', color: 'var(--color-error)' }}
                  onClick={() => {
                    setTempApiKey('');
                    apiServerClient.setApiKey('');
                    setApiKey('');
                    setShowApiKeyModal(false);
                    connectToServer();
                  }}
                >
                  {t('web.clear_key', { defaultValue: 'Clear Key' })}
                </button>
              )}
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setShowApiKeyModal(false)}
              >
                {t('common.cancel', { defaultValue: 'Cancel' })}
              </button>
              <button type="button" className="btn btn-primary btn-sm" onClick={handleSaveApiKey}>
                {t('common.save', { defaultValue: 'Save' })}
              </button>
            </div>
          </div>
        </div>
      )}
      {/* Global dialog containers */}
      <GlobalDialog />
      <ErrorDialog />
    </div>
  );
}
