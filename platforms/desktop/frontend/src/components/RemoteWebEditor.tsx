import { Key, Loader2, Server, Trash2 } from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { type ApiServerInfo, apiServerClient } from '../services/apiServerClient';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { exportToMarkdown, exportToSrt, exportToTxt, exportToVtt } from '../utils/webExport';
import { AudioPlayer } from './AudioPlayer';
import { Dropdown, type DropdownOption } from './Dropdown';
import { CloseIcon, DownloadIcon, FileTextIcon, UploadIcon } from './Icons';
import { TranscriptEditor } from './transcript/TranscriptEditor';

export function RemoteWebEditor(): React.JSX.Element {
  const { t } = useTranslation();

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

  const connectToServer = useCallback(async () => {
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
  }, [selectedModel]);

  useEffect(() => {
    connectToServer();
  }, [connectToServer]);

  // Clean up timer
  useEffect(() => {
    return () => {
      clearInterval(pollTimerRef.current as number);
    };
  }, []);
  // Click outside to close export menu
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (exportMenuRef.current && !exportMenuRef.current.contains(e.target as Node)) {
        setShowExportMenu(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const modelOptions: DropdownOption[] = (serverInfo?.models ?? []).map((m) => ({
    value: m.id,
    label: m.name || m.id,
  }));

  const languageOptions: DropdownOption[] = [
    { value: 'auto', label: '自动识别 (Auto)' },
    { value: 'zh', label: '中文 (Chinese)' },
    { value: 'en', label: '英语 (English)' },
    { value: 'ja', label: '日语 (Japanese)' },
    { value: 'ko', label: '韩语 (Korean)' },
    { value: 'yue', label: '粤语 (Cantonese)' },
  ];

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
    // Create local object URL for preview audio playback immediately
    const localAudioUrl = URL.createObjectURL(file);
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
    if (!selectedFile || !selectedModel) {
      return;
    }

    setIsTranscribing(true);
    setTranscribeProgress('正在上传音频并创建转录任务...');
    setTranscribeError(null);

    try {
      const jobId = await apiServerClient.transcribe(selectedFile, {
        modelId: selectedModel,
        language: selectedLanguage === 'auto' ? undefined : selectedLanguage,
      });

      setTranscribeProgress('任务已提交，正在等待排队处理...');

      clearInterval(pollTimerRef.current as number);

      pollTimerRef.current = window.setInterval(async () => {
        try {
          const status = await apiServerClient.getJobStatus(jobId);
          if (status === 'Pending') {
            setTranscribeProgress('排队等待处理中...');
          } else if (status === 'Processing') {
            setTranscribeProgress('正在转录处理中，请稍候...');
          } else if (typeof status === 'object' && 'Completed' in status) {
            clearInterval(pollTimerRef.current as number);
            pollTimerRef.current = null;
            setIsTranscribing(false);
            setTranscribeProgress(null);
            setSegments(status.Completed);

            // Update audio url to point to server audio endpoint
            const serverAudioUrl = apiServerClient.getAudioUrl(jobId);
            setAudioUrl(serverAudioUrl);
          } else if (typeof status === 'object' && 'Failed' in status) {
            clearInterval(pollTimerRef.current as number);
            pollTimerRef.current = null;
            setIsTranscribing(false);
            setTranscribeProgress(null);
            setTranscribeError(`转录失败: ${status.Failed}`);
          }
        } catch (pollErr: unknown) {
          clearInterval(pollTimerRef.current as number);
          pollTimerRef.current = null;
          setIsTranscribing(false);
          setTranscribeProgress(null);
          setTranscribeError(
            `查询状态失败: ${pollErr instanceof Error ? pollErr.message : String(pollErr)}`
          );
        }
      }, 1500);
    } catch (err: unknown) {
      setIsTranscribing(false);
      setTranscribeProgress(null);
      setTranscribeError(`发起转录失败: ${err instanceof Error ? err.message : String(err)}`);
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
    if (segments.length > 0 && !confirm('确定要清空当前的转录内容吗？')) {
      return;
    }
    clearActiveTranscriptSession();
    setSelectedFile(null);
    if (fileInputRef.current) {
      fileInputRef.current.value = '';
    }
  };

  const displayTitle = title || selectedFile?.name || '未命名转录';

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
            Web
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
            title="点击修改服务主机地址"
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
            title="配置 API Key 鉴权令牌"
          >
            <Key size={13} style={{ color: apiKey ? 'var(--color-success)' : undefined }} />
            <span>{apiKey ? 'API Key: 已设置' : 'API Key: 未设置'}</span>
          </button>
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
              <h2>{t('panel.batch_import', { defaultValue: '转录' })}</h2>
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
                      : t('batch.drop_title', { defaultValue: '拖入音频或点击上传' })}
                  </h3>
                  <p style={{ fontSize: '12px', margin: 0, color: 'var(--color-text-secondary)' }}>
                    {selectedFile
                      ? `${(selectedFile.size / (1024 * 1024)).toFixed(2)} MB`
                      : t('batch.drop_desc', {
                          defaultValue: '支持 MP3, WAV, M4A, FLAC, OGG 等常见格式',
                        })}
                  </p>
                </div>

                <div
                  className="btn btn-primary btn-sm"
                  style={{ marginTop: '8px', pointerEvents: 'none' }}
                  aria-hidden="true"
                >
                  {selectedFile
                    ? '更换文件'
                    : t('batch.select_file', { defaultValue: '选择音频文件' })}
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
                    {t('settings.asr.model', { defaultValue: 'ASR 模型' })}
                  </label>
                  <Dropdown
                    options={modelOptions}
                    value={selectedModel}
                    onChange={setSelectedModel}
                    placeholder={isConnected ? '选择模型...' : '请先连接服务'}
                    disabled={isTranscribing || !serverInfo?.models?.length}
                  />
                </div>

                <div className="form-group">
                  <label className="form-label" style={{ fontSize: '12px', marginBottom: '6px' }}>
                    {t('settings.asr.language', { defaultValue: '音频语言' })}
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
                  disabled={!selectedFile || isTranscribing || !isConnected}
                  onClick={handleStartTranscribe}
                >
                  {isTranscribing ? (
                    <>
                      <Loader2 size={15} className="animate-spin" />
                      <span>转录中...</span>
                    </>
                  ) : (
                    <>
                      <UploadIcon />
                      <span>开始转录</span>
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
                    {segments.length} 个句段
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
                        <span>导出</span>
                      </button>

                      {showExportMenu && (
                        <div className="export-dropdown">
                          <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={() => handleExport('srt')}
                          >
                            <span>SubRip (.srt)</span>
                          </button>
                          <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={() => handleExport('vtt')}
                          >
                            <span>WebVTT (.vtt)</span>
                          </button>
                          <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={() => handleExport('txt')}
                          >
                            <span>纯文本 (.txt)</span>
                          </button>
                          <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={() => handleExport('md')}
                          >
                            <span>Markdown (.md)</span>
                          </button>
                          <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={() => handleExport('json')}
                          >
                            <span>JSON 数据 (.json)</span>
                          </button>
                        </div>
                      )}
                    </div>

                    {/* Clear session */}
                    <button
                      type="button"
                      className="btn btn-icon btn-sm"
                      onClick={handleClearSession}
                      title="清空当前转录"
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
                    在左侧选择音频文件，点击“开始转录”以生成转录内容
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
              <h3>服务主机地址</h3>
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
                指定桌面端 Sona 运行的主机地址与端口（默认端口 14200）。
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
                取消
              </button>
              <button
                type="button"
                className="btn btn-primary btn-sm"
                onClick={handleSaveServerUrl}
              >
                保存并连接
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
              <h3>API Key 鉴权令牌</h3>
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
                如果桌面端开启了 API Key
                鉴权，请在此填入相同密钥。系统会自动将其保存至本地，并在转录请求及音频流中附加鉴权令牌。
              </p>
              <input
                type="password"
                className="input-text"
                value={tempApiKey}
                onChange={(e) => setTempApiKey(e.target.value)}
                placeholder="填入 API Key，留空表示无鉴权"
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
                  清除 Key
                </button>
              )}
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setShowApiKeyModal(false)}
              >
                取消
              </button>
              <button type="button" className="btn btn-primary btn-sm" onClick={handleSaveApiKey}>
                保存
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
