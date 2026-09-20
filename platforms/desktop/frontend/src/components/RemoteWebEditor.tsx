import {
  AlertCircle,
  Download,
  FileAudio,
  Languages,
  Loader2,
  Moon,
  RefreshCw,
  Server,
  Sparkles,
  Sun,
  Trash2,
  Upload,
} from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { type ApiServerInfo, apiServerClient } from '../services/apiServerClient';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import {
  downloadFile,
  exportToJson,
  exportToMarkdown,
  exportToSrt,
  exportToTxt,
  exportToVtt,
} from '../utils/webExport';
import { AudioPlayer } from './AudioPlayer';
import { TranscriptEditor } from './transcript/TranscriptEditor';

export function RemoteWebEditor(): React.JSX.Element {
  // Store bindings
  const segments = useTranscriptSessionStore((state) => state.segments);
  const title = useTranscriptSessionStore((state) => state.title);
  const openSession = useTranscriptStore((state) => state.openSession);
  const clearActiveSession = useTranscriptStore((state) => state.clearActiveTranscriptSession);
  const setSegments = useTranscriptStore((state) => state.setSegments);

  // Server state
  const [serverUrl, setServerUrl] = useState<string>(() => apiServerClient.getBaseUrl());
  const [isEditingServerUrl, setIsEditingServerUrl] = useState<boolean>(false);
  const [serverInfo, setServerInfo] = useState<ApiServerInfo | null>(null);
  const [isConnected, setIsConnected] = useState<boolean | null>(null);
  const [isConnecting, setIsConnecting] = useState<boolean>(false);

  // Transcription state
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [selectedModel, setSelectedModel] = useState<string>('');
  const [selectedLanguage, setSelectedLanguage] = useState<string>('auto');
  const [hotwords, setHotwords] = useState<string>('');
  const [isTranscribing, setIsTranscribing] = useState<boolean>(false);
  const [transcribeProgress, setTranscribeProgress] = useState<string>('');
  const [transcribeError, setTranscribeError] = useState<string | null>(null);

  // LLM action states
  const [isPolishing, setIsPolishing] = useState<boolean>(false);
  const [isTranslating, setIsTranslating] = useState<boolean>(false);
  const [translateLang, setTranslateLang] = useState<string>('zh');
  const [showTranslateModal, setShowTranslateModal] = useState<boolean>(false);

  // Export menu
  const [showExportMenu, setShowExportMenu] = useState<boolean>(false);
  const [exportMode, setExportMode] = useState<'original' | 'translation' | 'bilingual'>(
    'original'
  );

  // Dark mode
  const [isDarkMode, setIsDarkMode] = useState<boolean>(() => {
    if (typeof window !== 'undefined') {
      return (
        document.documentElement.classList.contains('dark') ||
        window.matchMedia('(prefers-color-scheme: dark)').matches
      );
    }
    return true;
  });

  const fileInputRef = useRef<HTMLInputElement>(null);

  // Sync dark mode class
  useEffect(() => {
    if (isDarkMode) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }, [isDarkMode]);

  // Connect to API server
  const connectToServer = useCallback(
    async (url?: string) => {
      const targetUrl = url ?? serverUrl;
      apiServerClient.setBaseUrl(targetUrl);
      setIsConnecting(true);
      setTranscribeError(null);
      try {
        await apiServerClient.checkHealth();
        const info = await apiServerClient.getInfo();
        setServerInfo(info);
        setIsConnected(true);
        if (info.models?.length > 0 && !selectedModel) {
          setSelectedModel(info.models[0].id);
        }
      } catch {
        setIsConnected(false);
        setServerInfo(null);
      } finally {
        setIsConnecting(false);
      }
    },
    [serverUrl, selectedModel]
  );

  useEffect(() => {
    connectToServer();
  }, [connectToServer]);

  // Handle file selection
  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      setSelectedFile(file);
      setTranscribeError(null);
    }
  };

  // Run transcription
  const handleTranscribe = async () => {
    if (!selectedFile) return;
    setIsTranscribing(true);
    setTranscribeProgress('Uploading audio file...');
    setTranscribeError(null);

    try {
      const jobId = await apiServerClient.transcribe(selectedFile, {
        modelId: selectedModel,
        language: selectedLanguage === 'auto' ? undefined : selectedLanguage,
        hotwords: hotwords.trim() || undefined,
      });

      setTranscribeProgress(`Processing on host (Job: ${jobId.slice(0, 8)})...`);

      // Poll for completion
      const pollInterval = setInterval(async () => {
        try {
          const status = await apiServerClient.getJobStatus(jobId);
          if (typeof status === 'object' && 'Completed' in status) {
            clearInterval(pollInterval);
            setIsTranscribing(false);
            setTranscribeProgress('');

            const localAudioUrl = URL.createObjectURL(selectedFile);
            openSession({
              segments: status.Completed,
              sourceHistoryId: null,
              title: selectedFile.name,
              audioUrl: localAudioUrl,
            });
          } else if (typeof status === 'object' && 'Failed' in status) {
            clearInterval(pollInterval);
            setIsTranscribing(false);
            setTranscribeError(`Transcription failed: ${status.Failed}`);
          }
        } catch (pollErr: unknown) {
          clearInterval(pollInterval);
          setIsTranscribing(false);
          const message = pollErr instanceof Error ? pollErr.message : String(pollErr);
          setTranscribeError(`Polling error: ${message}`);
        }
      }, 1000);
    } catch (err: unknown) {
      setIsTranscribing(false);
      const message = err instanceof Error ? err.message : String(err);
      setTranscribeError(message || 'Failed to submit transcription job');
    }
  };

  // Polish segments with AI
  const handlePolish = async () => {
    if (segments.length === 0 || isPolishing) return;
    setIsPolishing(true);
    try {
      const polished = await apiServerClient.polish(segments);
      setSegments(polished);
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      alert(`AI Polish failed: ${message}`);
    } finally {
      setIsPolishing(false);
    }
  };

  // Translate segments with AI
  const handleTranslate = async () => {
    if (segments.length === 0 || isTranslating) return;
    setIsTranslating(true);
    setShowTranslateModal(false);
    try {
      const translated = await apiServerClient.translate(segments, translateLang);
      setSegments(translated);
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      alert(`AI Translate failed: ${message}`);
    } finally {
      setIsTranslating(false);
    }
  };

  // Export handlers
  const handleExport = (format: 'srt' | 'vtt' | 'txt' | 'json' | 'md') => {
    if (segments.length === 0) return;
    const baseName = (title || 'transcript').replace(/\.[^/.]+$/, '');
    let content = '';
    let ext = format;
    let mime = 'text/plain';

    switch (format) {
      case 'srt':
        content = exportToSrt(segments, exportMode);
        mime = 'application/x-subrip';
        break;
      case 'vtt':
        content = exportToVtt(segments, exportMode);
        mime = 'text/vtt';
        break;
      case 'txt':
        content = exportToTxt(segments, exportMode);
        break;
      case 'json':
        content = exportToJson(segments);
        ext = 'json';
        mime = 'application/json';
        break;
      case 'md':
        content = exportToMarkdown(segments, exportMode);
        ext = 'md';
        mime = 'text/markdown';
        break;
    }
    downloadFile(content, `${baseName}.${ext}`, mime);
    setShowExportMenu(false);
  };

  const handleClearSession = () => {
    if (segments.length > 0 && !confirm('Clear current transcript session?')) {
      return;
    }
    clearActiveSession();
    setSelectedFile(null);
    if (fileInputRef.current) {
      fileInputRef.current.value = '';
    }
  };

  return (
    <div className="sona-web-editor">
      {/* Header */}
      <header className="sona-web-header">
        <div className="sona-web-header-brand">
          <div className="sona-web-logo-badge">
            <span className="sona-web-logo-icon">S</span>
            <span>Sona Web</span>
            <span className="sona-web-tag">Remote</span>
          </div>

          {/* Host connection pill */}
          <div className="sona-web-server-pill">
            <Server size={13} style={{ color: 'var(--color-text-muted)' }} />
            {isEditingServerUrl ? (
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  setIsEditingServerUrl(false);
                  connectToServer();
                }}
                style={{ display: 'flex', alignItems: 'center', gap: '4px' }}
              >
                <input
                  type="text"
                  value={serverUrl}
                  onChange={(e) => setServerUrl(e.target.value)}
                  className="sona-web-server-input"
                  autoFocus
                />
                <button
                  type="submit"
                  className="btn btn-sm btn-primary"
                  style={{ padding: '2px 6px', height: '22px' }}
                >
                  Save
                </button>
              </form>
            ) : (
              <button
                type="button"
                onClick={() => setIsEditingServerUrl(true)}
                className="sona-web-server-url-btn"
                title="Click to edit server address"
              >
                <span>{serverUrl}</span>
              </button>
            )}

            <span
              className={`sona-web-status-dot ${
                isConnected === true
                  ? 'connected'
                  : isConnected === false
                    ? 'disconnected'
                    : 'connecting'
              }`}
              title={
                isConnected === true
                  ? 'Connected to Sona Desktop'
                  : isConnected === false
                    ? 'Disconnected from Sona Desktop'
                    : 'Checking connection...'
              }
            />
            <button
              type="button"
              onClick={() => connectToServer()}
              disabled={isConnecting}
              className="btn-icon"
              style={{ width: '20px', height: '20px' }}
              title="Refresh connection"
            >
              <RefreshCw size={12} className={isConnecting ? 'animate-spin' : ''} />
            </button>
          </div>
        </div>

        {/* Right actions */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <button
            type="button"
            onClick={() => setIsDarkMode(!isDarkMode)}
            className="btn-icon"
            title="Toggle Theme"
          >
            {isDarkMode ? <Sun size={16} /> : <Moon size={16} />}
          </button>
        </div>
      </header>

      {/* Main Content Area */}
      <div className="sona-web-body">
        {/* Left Controls Sidebar */}
        <aside className="sona-web-sidebar">
          <div>
            <div className="sona-web-section-title">Audio Source</div>

            <input
              type="file"
              ref={fileInputRef}
              onChange={handleFileChange}
              accept="audio/*,video/*"
              style={{ display: 'none' }}
              id="web-audio-input"
            />

            <label htmlFor="web-audio-input" className="sona-web-file-dropzone">
              <FileAudio size={28} style={{ color: 'var(--color-text-muted)' }} />
              <span style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>
                {selectedFile ? (
                  <span
                    style={{
                      fontWeight: 500,
                      color: 'var(--color-text-primary)',
                      wordBreak: 'break-all',
                    }}
                  >
                    {selectedFile.name} ({(selectedFile.size / 1024 / 1024).toFixed(1)} MB)
                  </span>
                ) : (
                  <span>Select or drop audio/video file</span>
                )}
              </span>
              <span style={{ fontSize: '10px', color: 'var(--color-text-muted)' }}>
                Supports MP3, WAV, M4A, FLAC, MP4, etc.
              </span>
            </label>
          </div>

          {/* Model & Language settings */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
            <div className="sona-web-form-group">
              <label className="sona-web-label">ASR Model</label>
              <select
                value={selectedModel}
                onChange={(e) => setSelectedModel(e.target.value)}
                disabled={isTranscribing || !serverInfo?.models?.length}
                className="sona-web-select"
              >
                {serverInfo?.models?.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name || m.id} {m.installed ? '(Installed)' : ''}
                  </option>
                )) || <option value="">No models discovered</option>}
              </select>
            </div>

            <div className="sona-web-form-group">
              <label className="sona-web-label">Language</label>
              <select
                value={selectedLanguage}
                onChange={(e) => setSelectedLanguage(e.target.value)}
                disabled={isTranscribing}
                className="sona-web-select"
              >
                <option value="auto">Auto Detect</option>
                <option value="zh">Chinese (中文)</option>
                <option value="en">English</option>
                <option value="ja">Japanese (日本語)</option>
                <option value="ko">Korean (한국어)</option>
                <option value="yue">Cantonese (粤语)</option>
              </select>
            </div>

            <div className="sona-web-form-group">
              <label className="sona-web-label">Hotwords (Optional)</label>
              <input
                type="text"
                placeholder="Comma-separated keywords"
                value={hotwords}
                onChange={(e) => setHotwords(e.target.value)}
                disabled={isTranscribing}
                className="sona-web-input"
              />
            </div>
          </div>

          {/* Start Transcription Action */}
          <button
            type="button"
            onClick={handleTranscribe}
            disabled={!selectedFile || isTranscribing || !isConnected}
            className="btn btn-primary"
            style={{ width: '100%' }}
          >
            {isTranscribing ? (
              <>
                <Loader2 size={14} className="animate-spin" />
                <span>Transcribing...</span>
              </>
            ) : (
              <>
                <Upload size={14} />
                <span>Start Transcription</span>
              </>
            )}
          </button>

          {/* Progress / Status */}
          {transcribeProgress && (
            <div className="sona-web-progress-box">
              <Loader2 size={13} className="animate-spin" />
              <span>{transcribeProgress}</span>
            </div>
          )}

          {transcribeError && (
            <div className="sona-web-error-box">
              <AlertCircle size={14} style={{ flexShrink: 0, marginTop: '2px' }} />
              <span>{transcribeError}</span>
            </div>
          )}

          {/* Session details */}
          {segments.length > 0 && (
            <div
              style={{
                marginTop: 'auto',
                paddingTop: '16px',
                borderTop: '1px solid var(--color-border)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                fontSize: '12px',
                color: 'var(--color-text-muted)',
              }}
            >
              <span>{segments.length} segments</span>
              <button
                type="button"
                onClick={handleClearSession}
                className="btn btn-sm btn-text"
                style={{ color: 'var(--color-error)' }}
                title="Clear current session"
              >
                <Trash2 size={13} />
                <span>Clear</span>
              </button>
            </div>
          )}
        </aside>

        {/* Right Editor Workspace */}
        <main className="sona-web-main">
          {/* Top Workbench Toolbar */}
          <div className="sona-web-toolbar">
            <div className="sona-web-toolbar-title">
              {title || (selectedFile ? selectedFile.name : 'Untitled Session')}
            </div>

            {/* Editing and AI action buttons */}
            <div className="sona-web-toolbar-actions">
              <button
                type="button"
                onClick={handlePolish}
                disabled={segments.length === 0 || isPolishing}
                className="btn btn-sm btn-secondary"
                title="AI Polish (Refine punctuation & grammar)"
              >
                <Sparkles size={13} style={{ color: 'var(--color-accent-primary)' }} />
                <span>{isPolishing ? 'Polishing...' : 'Polish'}</span>
              </button>

              <button
                type="button"
                onClick={() => setShowTranslateModal(true)}
                disabled={segments.length === 0 || isTranslating}
                className="btn btn-sm btn-secondary"
                title="AI Translate"
              >
                <Languages size={13} style={{ color: 'var(--color-accent-primary)' }} />
                <span>{isTranslating ? 'Translating...' : 'Translate'}</span>
              </button>

              {/* Export Dropdown */}
              <div style={{ position: 'relative' }}>
                <button
                  type="button"
                  onClick={() => setShowExportMenu(!showExportMenu)}
                  disabled={segments.length === 0}
                  className="btn btn-sm btn-secondary"
                >
                  <Download size={13} />
                  <span>Export</span>
                </button>

                {showExportMenu && (
                  <div className="sona-web-dropdown-menu">
                    <div className="sona-web-dropdown-header">
                      <span>Mode</span>
                      <div className="sona-web-dropdown-modes">
                        {(['original', 'translation', 'bilingual'] as const).map((m) => (
                          <button
                            key={m}
                            type="button"
                            onClick={() => setExportMode(m)}
                            className={`sona-web-mode-btn ${exportMode === m ? 'active' : ''}`}
                          >
                            {m[0].toUpperCase()}
                          </button>
                        ))}
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={() => handleExport('srt')}
                      className="sona-web-dropdown-item"
                    >
                      SubRip Subtitle (.srt)
                    </button>
                    <button
                      type="button"
                      onClick={() => handleExport('vtt')}
                      className="sona-web-dropdown-item"
                    >
                      WebVTT Subtitle (.vtt)
                    </button>
                    <button
                      type="button"
                      onClick={() => handleExport('txt')}
                      className="sona-web-dropdown-item"
                    >
                      Plain Text (.txt)
                    </button>
                    <button
                      type="button"
                      onClick={() => handleExport('md')}
                      className="sona-web-dropdown-item"
                    >
                      Markdown (.md)
                    </button>
                    <button
                      type="button"
                      onClick={() => handleExport('json')}
                      className="sona-web-dropdown-item"
                    >
                      JSON Data (.json)
                    </button>
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Audio Player Bar */}
          <AudioPlayer />

          {/* Editor Area */}
          <div className="sona-web-editor-container">
            {segments.length > 0 ? (
              <TranscriptEditor />
            ) : (
              <div className="sona-web-empty-state">
                <FileAudio size={48} style={{ marginBottom: '12px', opacity: 0.3 }} />
                <p
                  style={{
                    fontSize: '14px',
                    fontWeight: 500,
                    color: 'var(--color-text-primary)',
                    marginBottom: '4px',
                  }}
                >
                  No transcript loaded
                </p>
                <p style={{ fontSize: '12px', maxWidth: '360px' }}>
                  Upload an audio file on the left and click "Start Transcription" to begin editing.
                </p>
              </div>
            )}
          </div>
        </main>
      </div>

      {/* Translate Language Selection Modal */}
      {showTranslateModal && (
        <div className="shared-modal-overlay">
          <div className="shared-modal-shell shared-modal-sm">
            <div className="shared-modal-header">
              <h3
                className="shared-modal-title"
                style={{ display: 'flex', alignItems: 'center', gap: '8px' }}
              >
                <Languages size={18} />
                <span>Translate Transcript</span>
              </h3>
            </div>

            <div className="shared-modal-body">
              <div className="sona-web-form-group">
                <label className="sona-web-label">Target Language</label>
                <select
                  value={translateLang}
                  onChange={(e) => setTranslateLang(e.target.value)}
                  className="sona-web-select"
                >
                  <option value="zh">Chinese (中文)</option>
                  <option value="en">English</option>
                  <option value="ja">Japanese (日本語)</option>
                  <option value="ko">Korean (한국어)</option>
                  <option value="es">Spanish (Español)</option>
                  <option value="fr">French (Français)</option>
                  <option value="de">German (Deutsch)</option>
                </select>
              </div>
            </div>

            <div className="shared-modal-footer">
              <button
                type="button"
                onClick={() => setShowTranslateModal(false)}
                className="btn btn-sm btn-secondary"
              >
                Cancel
              </button>
              <button type="button" onClick={handleTranslate} className="btn btn-sm btn-primary">
                Translate
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
