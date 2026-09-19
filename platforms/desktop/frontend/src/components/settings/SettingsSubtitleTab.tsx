import {
  Check,
  Copy,
  History,
  Keyboard,
  Plus,
  SlidersHorizontal,
  Subtitles,
  Trash2,
  X,
} from 'lucide-react';
import type React from 'react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useVoiceTypingReadiness } from '../../hooks/useVoiceTypingReadiness';
import {
  useCaptionConfig,
  useConfigStore,
  useSetConfig,
  useVoiceTypingConfig,
} from '../../stores/configStore';
import { useVoiceTypingHistoryStore } from '../../stores/voiceTypingHistoryStore';
import type { VoiceTypingRuntimeErrorSource } from '../../stores/voiceTypingRuntimeStore';
import { logger } from '../../utils/logger';
import { ColorSwatchPicker } from '../ColorSwatchPicker';
import { Dropdown } from '../Dropdown';
import { SubtitleIcon } from '../Icons';
import { Switch } from '../Switch';
import {
  SettingsItem,
  SettingsPageHeader,
  SettingsSection,
  SettingsTabContainer,
} from './SettingsLayout';
import { SettingsShortcutInput } from './SettingsShortcutInput';

type AvailabilityTone = 'ready' | 'off' | 'missing';

function StatusBadge({
  tone,
  label,
}: {
  tone: AvailabilityTone;
  label: string;
}): React.JSX.Element {
  const icon = tone === 'ready' ? <Check size={12} /> : <X size={12} />;

  return (
    <span className={`status-badge ${tone}`}>
      {icon}
      {label}
    </span>
  );
}

function getFailureSourceLabel(
  t: (key: string, options?: Record<string, unknown>) => string,
  source: VoiceTypingRuntimeErrorSource | null
) {
  switch (source) {
    case 'shortcut_registration':
      return t('settings.voice_typing_failure_source_shortcut', {
        defaultValue: 'Shortcut registration',
      });
    case 'warmup':
      return t('settings.voice_typing_failure_source_warmup', {
        defaultValue: 'Background warm-up',
      });
    case 'microphone':
      return t('settings.voice_typing_failure_source_microphone', {
        defaultValue: 'Input device',
      });
    case 'session':
      return t('settings.voice_typing_failure_source_session', {
        defaultValue: 'Voice Typing session',
      });
    default:
      return null;
  }
}

function VoiceTypingSettingsSection(): React.JSX.Element {
  const { t } = useTranslation();
  const vtConfig = useVoiceTypingConfig();
  const updateConfig = useSetConfig();
  const readiness = useVoiceTypingReadiness();
  const isAvailable = readiness.state === 'ready';
  const hasFailureReason = readiness.state === 'failed' && Boolean(readiness.lastErrorMessage);
  const failureSourceLabel = getFailureSourceLabel(t, readiness.lastErrorSource);
  const availabilityTone: AvailabilityTone = isAvailable
    ? 'ready'
    : hasFailureReason
      ? 'missing'
      : 'off';

  return (
    <SettingsSection
      title={t('settings.voice_typing', { defaultValue: 'Voice Typing' })}
      icon={<Keyboard size={20} />}
    >
      <SettingsItem
        title={t('settings.enable_voice_typing', {
          defaultValue: 'Enable Voice Typing',
        })}
        hint={t('settings.enable_voice_typing_hint', {
          defaultValue: 'Type text directly into any application using your voice',
        })}
      >
        <Switch
          checked={vtConfig.voiceTypingEnabled ?? false}
          onChange={(val) => updateConfig({ voiceTypingEnabled: val })}
        />
      </SettingsItem>

      <SettingsItem
        title={t('settings.voice_typing_shortcut', { defaultValue: 'Shortcut' })}
        hint={t('settings.voice_typing_shortcut_hint', {
          defaultValue: 'Global shortcut to activate voice typing',
        })}
      >
        <SettingsShortcutInput
          value={vtConfig.voiceTypingShortcut ?? 'Alt+V'}
          onChange={(val) => updateConfig({ voiceTypingShortcut: val })}
        />
      </SettingsItem>

      <SettingsItem
        title={t('settings.voice_typing_mode', { defaultValue: 'Mode' })}
        hint={t('settings.voice_typing_mode_hint', {
          defaultValue: 'How the shortcut triggers listening',
        })}
      >
        <div style={{ width: '180px' }}>
          <Dropdown
            id="vt-mode-select"
            value={vtConfig.voiceTypingMode || 'hold'}
            onChange={(val) => updateConfig({ voiceTypingMode: val as 'hold' | 'toggle' })}
            options={[
              {
                value: 'hold',
                label: t('settings.voice_typing_mode_hold', {
                  defaultValue: 'Push to Talk (Hold)',
                }),
              },
              {
                value: 'toggle',
                label: t('settings.voice_typing_mode_toggle', {
                  defaultValue: 'Toggle (Press once)',
                }),
              },
            ]}
          />
        </div>
      </SettingsItem>

      <SettingsItem
        title={t('settings.voice_typing_processing_mode', { defaultValue: 'Processing Mode' })}
        hint={t('settings.voice_typing_processing_mode_hint', {
          defaultValue: 'Choose between fast raw output or AI-powered smart polishing',
        })}
      >
        <div style={{ width: '220px' }}>
          <Dropdown
            id="vt-processing-mode-select"
            value={vtConfig.voiceTypingProcessingMode || 'raw'}
            onChange={(val) =>
              updateConfig({
                voiceTypingProcessingMode: val as 'raw' | 'polish',
              })
            }
            options={[
              {
                value: 'raw',
                label: t('settings.voice_typing_processing_mode_raw', {
                  defaultValue: 'Fast Dictation (Raw)',
                }),
              },
              {
                value: 'polish',
                label: t('settings.voice_typing_processing_mode_polish', {
                  defaultValue: 'Smart Polish (AI Rewrite)',
                }),
              },
            ]}
          />
        </div>
      </SettingsItem>

      {vtConfig.voiceTypingProcessingMode === 'polish' && (
        <SettingsItem
          title={t('settings.voice_typing_polish_prompt', { defaultValue: 'Custom Polish Prompt' })}
          hint={t('settings.voice_typing_polish_prompt_hint', {
            defaultValue:
              'Leave blank to use the built-in fast colloquial-to-written prompt directive',
          })}
        >
          <textarea
            className="settings-input"
            rows={3}
            style={{ width: '100%', maxWidth: '400px', resize: 'vertical' }}
            placeholder={t('settings.voice_typing_polish_prompt_hint', {
              defaultValue:
                'Leave blank to use the built-in fast colloquial-to-written prompt directive',
            })}
            value={vtConfig.voiceTypingPolishPrompt ?? ''}
            onChange={(e) => updateConfig({ voiceTypingPolishPrompt: e.target.value })}
          />
        </SettingsItem>
      )}

      <SettingsItem
        title={t('settings.voice_typing_sound_enabled', {
          defaultValue: 'Audio Feedback (Earcons)',
        })}
        hint={t('settings.voice_typing_sound_enabled_hint', {
          defaultValue: 'Play sound cues on start, commit, cancel, or error',
        })}
      >
        <Switch
          checked={vtConfig.voiceTypingSoundEnabled ?? true}
          onChange={(val) => updateConfig({ voiceTypingSoundEnabled: val })}
        />
      </SettingsItem>

      <SettingsItem
        title={t('settings.voice_typing_cjk_spacing_enabled', {
          defaultValue: 'CJK-Latin Typography Spacing',
        })}
        hint={t('settings.voice_typing_cjk_spacing_enabled_hint', {
          defaultValue:
            'Automatically insert spaces between CJK and Latin characters/numbers and harmonize punctuation',
        })}
      >
        <Switch
          checked={vtConfig.voiceTypingCjkSpacingEnabled ?? true}
          onChange={(val) => updateConfig({ voiceTypingCjkSpacingEnabled: val })}
        />
      </SettingsItem>
      <SettingsItem
        title={t('settings.voice_typing_availability', {
          defaultValue: 'Availability',
        })}
        hint={(() => {
          if (!hasFailureReason) {
            return undefined;
          }

          if (failureSourceLabel) {
            return t('settings.voice_typing_failure_reason_with_source', {
              defaultValue: 'Failure reason: {{source}}: {{message}}',
              source: failureSourceLabel,
              message: readiness.lastErrorMessage,
            });
          }

          return t('settings.voice_typing_failure_reason', {
            defaultValue: 'Failure reason: {{message}}',
            message: readiness.lastErrorMessage,
          });
        })()}
      >
        <StatusBadge
          tone={availabilityTone}
          label={
            isAvailable
              ? t('settings.voice_typing_available', { defaultValue: 'Available' })
              : t('settings.voice_typing_unavailable', { defaultValue: 'Unavailable' })
          }
        />
      </SettingsItem>
    </SettingsSection>
  );
}

function VoiceTypingHistorySection(): React.JSX.Element {
  const { t } = useTranslation();
  const historyItems = useVoiceTypingHistoryStore((state) => state.items);
  const removeItem = useVoiceTypingHistoryStore((state) => state.removeItem);
  const clearHistory = useVoiceTypingHistoryStore((state) => state.clearHistory);
  const updateConfig = useSetConfig();
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [addedHotwordId, setAddedHotwordId] = useState<string | null>(null);

  const handleCopy = async (id: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedId(id);
      setTimeout(() => setCopiedId((current) => (current === id ? null : current)), 1500);
    } catch (err) {
      logger.warn('[VoiceTypingHistory] Failed to copy to clipboard', err);
    }
  };

  const handleAddToHotwords = (id: string, text: string) => {
    const trimmed = text.trim();
    if (!trimmed) return;
    const config = useConfigStore.getState().config;
    const existingSets = config.hotwordSets || [];
    let updated = false;

    const nextSets = existingSets.map((set) => {
      if (set.enabled && !updated) {
        updated = true;
        return {
          ...set,
          rules: [...set.rules, { id: `hw_${Date.now()}`, text: trimmed }],
        };
      }
      return set;
    });

    if (!updated) {
      nextSets.push({
        id: `hw_set_${Date.now()}`,
        name: t('settings.voice_typing_hotwords_set_name', { defaultValue: '语音输入热词' }),
        enabled: true,
        rules: [{ id: `hw_${Date.now()}`, text: trimmed }],
      });
    }

    updateConfig({ hotwordSets: nextSets });
    setAddedHotwordId(id);
    setTimeout(() => setAddedHotwordId((current) => (current === id ? null : current)), 1500);
  };

  return (
    <SettingsSection
      title={t('settings.voice_typing_history', { defaultValue: 'Dictation History' })}
      icon={<History size={20} />}
    >
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '8px',
          gap: '12px',
        }}
      >
        <span style={{ fontSize: '13px', color: 'var(--color-text-muted)' }}>
          {t('settings.voice_typing_history_hint', {
            defaultValue: 'Recent voice typing entries are saved here to prevent text loss.',
          })}
        </span>
        {historyItems.length > 0 && (
          <button
            type="button"
            onClick={clearHistory}
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: '6px',
              padding: '4px 10px',
              fontSize: '12px',
              borderRadius: 'var(--radius-sm, 6px)',
              border: '1px solid var(--color-border)',
              background: 'transparent',
              color: 'var(--color-text-muted)',
              cursor: 'pointer',
              flexShrink: 0,
            }}
          >
            <Trash2 size={13} />
            {t('common.clear', { defaultValue: 'Clear' })}
          </button>
        )}
      </div>

      {historyItems.length === 0 ? (
        <div
          data-testid="voice-typing-history-empty"
          style={{
            padding: '24px 16px',
            textAlign: 'center',
            color: 'var(--color-text-muted)',
            fontSize: '13px',
            borderRadius: 'var(--radius-md, 8px)',
            background: 'var(--color-bg-secondary)',
            border: '1px dashed var(--color-border)',
          }}
        >
          {t('settings.voice_typing_history_empty', {
            defaultValue:
              'No dictation history yet. Texts transcribed via voice typing will appear here.',
          })}
        </div>
      ) : (
        <div
          data-testid="voice-typing-history-list"
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '8px',
            maxHeight: '420px',
            overflowY: 'auto',
          }}
        >
          {historyItems.map((item) => (
            <div
              key={item.id}
              data-testid={`voice-typing-history-item-${item.id}`}
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: '6px',
                padding: '12px 14px',
                borderRadius: 'var(--radius-md, 8px)',
                background: 'var(--color-bg-secondary)',
                border: '1px solid var(--color-border)',
              }}
            >
              <div
                style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <span
                    style={{
                      fontSize: '11px',
                      fontWeight: 600,
                      padding: '2px 6px',
                      borderRadius: '4px',
                      background:
                        item.mode === 'polish'
                          ? 'rgba(168, 85, 247, 0.15)'
                          : 'rgba(59, 130, 246, 0.15)',
                      color:
                        item.mode === 'polish'
                          ? 'var(--color-accent-purple, #a855f7)'
                          : 'var(--color-accent-blue, #3b82f6)',
                    }}
                  >
                    {item.mode === 'polish'
                      ? t('settings.voice_typing_mode_badge_polish', { defaultValue: 'AI 润色' })
                      : t('settings.voice_typing_mode_badge_raw', { defaultValue: '极速直出' })}
                  </span>
                  <span style={{ fontSize: '11px', color: 'var(--color-text-muted)' }}>
                    {new Date(item.timestamp).toLocaleTimeString()}
                  </span>
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                  <button
                    type="button"
                    title={t('common.copy', { defaultValue: 'Copy' })}
                    onClick={() => void handleCopy(item.id, item.injectedText)}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: '4px',
                      padding: '3px 8px',
                      fontSize: '12px',
                      borderRadius: '4px',
                      border: '1px solid var(--color-border)',
                      background: 'var(--color-bg-elevated)',
                      color: 'var(--color-text-primary)',
                      cursor: 'pointer',
                    }}
                  >
                    {copiedId === item.id ? (
                      <Check size={12} color="#22c55e" />
                    ) : (
                      <Copy size={12} />
                    )}
                    {copiedId === item.id
                      ? t('common.copied', { defaultValue: 'Copied' })
                      : t('common.copy', { defaultValue: 'Copy' })}
                  </button>
                  <button
                    type="button"
                    title={t('settings.voice_typing_add_hotword', {
                      defaultValue: 'Add to Hotwords',
                    })}
                    onClick={() => handleAddToHotwords(item.id, item.injectedText)}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: '4px',
                      padding: '3px 8px',
                      fontSize: '12px',
                      borderRadius: '4px',
                      border: '1px solid var(--color-border)',
                      background: 'var(--color-bg-elevated)',
                      color: 'var(--color-text-primary)',
                      cursor: 'pointer',
                    }}
                  >
                    {addedHotwordId === item.id ? (
                      <Check size={12} color="#22c55e" />
                    ) : (
                      <Plus size={12} />
                    )}
                    {addedHotwordId === item.id
                      ? t('settings.voice_typing_hotword_added', { defaultValue: '已添加' })
                      : t('settings.voice_typing_add_hotword', { defaultValue: '加为热词' })}
                  </button>
                  <button
                    type="button"
                    title={t('common.delete', { defaultValue: 'Delete' })}
                    onClick={() => removeItem(item.id)}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      padding: '4px',
                      borderRadius: '4px',
                      border: 'none',
                      background: 'transparent',
                      color: 'var(--color-text-muted)',
                      cursor: 'pointer',
                    }}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>
              <div
                style={{
                  fontSize: '13px',
                  lineHeight: '1.5',
                  color: 'var(--color-text-primary)',
                  wordBreak: 'break-word',
                  userSelect: 'text',
                }}
              >
                {item.injectedText}
              </div>
              {item.mode === 'polish' && item.rawText && item.rawText !== item.injectedText && (
                <div
                  style={{
                    fontSize: '12px',
                    color: 'var(--color-text-muted)',
                    fontStyle: 'italic',
                    background: 'var(--color-bg-tertiary, rgba(0,0,0,0.03))',
                    padding: '4px 8px',
                    borderRadius: '4px',
                  }}
                >
                  {t('settings.voice_typing_original_text', { defaultValue: '原识别草稿' })}:{' '}
                  {item.rawText}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </SettingsSection>
  );
}
export type SubtitleSubTab = 'voice_typing' | 'subtitles';

export interface SettingsSubtitleTabProps {
  initialSubTab?: SubtitleSubTab;
}

export function SettingsSubtitleTab({
  initialSubTab = 'voice_typing',
}: SettingsSubtitleTabProps = {}): React.JSX.Element {
  const { t } = useTranslation();
  const config = useCaptionConfig();
  const updateConfig = useSetConfig();

  const [activeSubTab, setActiveSubTab] = useState<SubtitleSubTab>(initialSubTab);

  useEffect(() => {
    if (initialSubTab) {
      setActiveSubTab(initialSubTab);
    }
  }, [initialSubTab]);

  const handleTabKeyDown = (e: React.KeyboardEvent) => {
    const tabs: SubtitleSubTab[] = ['voice_typing', 'subtitles'];
    const currentIndex = tabs.indexOf(activeSubTab);
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      const nextTab = tabs[(currentIndex + 1) % tabs.length];
      setActiveSubTab(nextTab);
      document.getElementById(`settings-subtitle-subtab-${nextTab}`)?.focus();
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      const prevTab = tabs[(currentIndex - 1 + tabs.length) % tabs.length];
      setActiveSubTab(prevTab);
      document.getElementById(`settings-subtitle-subtab-${prevTab}`)?.focus();
    }
  };

  const lockWindow = config.lockWindow ?? false;
  const alwaysOnTop = config.alwaysOnTop ?? true;
  const startOnLaunch = config.startOnLaunch ?? false;
  const captionWindowWidth = config.captionWindowWidth ?? 800;
  const captionFontSize = config.captionFontSize ?? 24;
  const captionFontColor = config.captionFontColor || '#ffffff';
  const captionBackgroundColor = config.captionBackgroundColor || '#000000';
  const captionBackgroundOpacity = config.captionBackgroundOpacity ?? 0.6;

  return (
    <SettingsTabContainer id="settings-panel-subtitle" ariaLabelledby="settings-tab-subtitle">
      <SettingsPageHeader
        icon={<SubtitleIcon width={28} height={28} />}
        title={t('settings.subtitle_voice_typing_title', {
          defaultValue: 'Subtitles & Voice Typing',
        })}
        description={t('settings.subtitle_voice_typing_desc', {
          defaultValue: 'Configure the live caption window and voice typing into other apps.',
        })}
      />
      <div
        id="settings-subtitle-categories"
        className="settings-scenario-cards"
        role="tablist"
        aria-label={t('settings.subtitle_voice_typing_title', {
          defaultValue: 'Subtitles & Voice Typing',
        })}
        onKeyDown={handleTabKeyDown}
        style={{
          border: '1px solid var(--color-border)',
          borderRadius: 'var(--radius-lg, 16px)',
          background: 'var(--color-bg-primary)',
          boxShadow: 'var(--shadow-xs, 0 1px 2px rgba(0, 0, 0, 0.04))',
        }}
      >
        {[
          {
            value: 'voice_typing' as const,
            label: t('settings.voice_typing', { defaultValue: 'Voice Typing' }),
            description: t('settings.voice_typing_description', {
              defaultValue:
                'Configure dictation into other applications and see whether Voice Typing is ready to run.',
            }),
            icon: <Keyboard size={18} />,
          },
          {
            value: 'subtitles' as const,
            label: t('live.subtitle_settings', { defaultValue: 'Subtitle Settings' }),
            description: t('settings.subtitle_tab_description', {
              defaultValue: 'Configure live caption window behavior and visual appearance.',
            }),
            icon: <Subtitles size={18} />,
          },
        ].map((tab) => (
          <button
            id={`settings-subtitle-subtab-${tab.value}`}
            key={tab.value}
            type="button"
            role="tab"
            aria-selected={activeSubTab === tab.value}
            aria-controls={`settings-subtitle-panel-${tab.value}`}
            aria-label={tab.label}
            tabIndex={activeSubTab === tab.value ? 0 : -1}
            className={`settings-scenario-card${activeSubTab === tab.value ? ' active' : ''}`}
            onClick={() => setActiveSubTab(tab.value)}
          >
            <span className="settings-scenario-card-icon">{tab.icon}</span>
            <span className="settings-scenario-card-text">
              <span className="settings-scenario-card-label">{tab.label}</span>
              <span className="settings-scenario-card-description">{tab.description}</span>
            </span>
          </button>
        ))}
      </div>

      {activeSubTab === 'voice_typing' && (
        <div
          id="settings-subtitle-panel-voice_typing"
          role="tabpanel"
          aria-labelledby="settings-subtitle-subtab-voice_typing"
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: 'var(--spacing-xl, 32px)',
            animation: 'fadeIn var(--transition-normal, 0.2s) ease-in-out',
          }}
        >
          <VoiceTypingSettingsSection />
          <VoiceTypingHistorySection />
        </div>
      )}

      {activeSubTab === 'subtitles' && (
        <div
          id="settings-subtitle-panel-subtitles"
          role="tabpanel"
          aria-labelledby="settings-subtitle-subtab-subtitles"
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: 'var(--spacing-xl, 32px)',
            animation: 'fadeIn var(--transition-normal, 0.2s) ease-in-out',
          }}
        >
          <SettingsSection
            title={t('settings.subtitle_behavior_title')}
            icon={<SlidersHorizontal size={20} />}
          >
            <SettingsItem title={t('live.start_on_launch')} hint={t('live.start_on_launch_hint')}>
              <Switch
                checked={startOnLaunch}
                onChange={(enabled) => updateConfig({ startOnLaunch: enabled })}
              />
            </SettingsItem>

            <SettingsItem title={t('live.always_on_top')} hint={t('live.always_on_top_hint')}>
              <Switch
                checked={alwaysOnTop}
                onChange={(enabled) => updateConfig({ alwaysOnTop: enabled })}
              />
            </SettingsItem>

            <SettingsItem title={t('live.lock_window')} hint={t('live.lock_window_hint')}>
              <Switch
                checked={lockWindow}
                onChange={(enabled) => updateConfig({ lockWindow: enabled })}
              />
            </SettingsItem>
          </SettingsSection>

          <SettingsSection
            title={t('settings.subtitle_appearance_title')}
            icon={<Subtitles size={20} />}
            description={t('settings.subtitle_appearance_desc')}
          >
            <SettingsItem title={t('live.window_width')}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <input
                  type="number"
                  min="300"
                  max="1600"
                  step="50"
                  value={captionWindowWidth}
                  onChange={(e) => updateConfig({ captionWindowWidth: Number(e.target.value) })}
                  className="settings-input"
                  style={{ width: '100px', textAlign: 'center' }}
                />
              </div>
            </SettingsItem>

            <SettingsItem title={t('live.font_size')}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <input
                  type="number"
                  min="12"
                  max="72"
                  step="1"
                  value={captionFontSize}
                  onChange={(e) => updateConfig({ captionFontSize: Number(e.target.value) })}
                  className="settings-input"
                  style={{ width: '100px', textAlign: 'center' }}
                />
              </div>
            </SettingsItem>

            <SettingsItem title={t('live.font_color')}>
              <ColorSwatchPicker
                value={captionFontColor}
                onChange={(color) => updateConfig({ captionFontColor: color })}
                aria-label={t('live.font_color')}
              />
            </SettingsItem>

            <SettingsItem title={t('live.background_color')}>
              <ColorSwatchPicker
                value={captionBackgroundColor}
                onChange={(color) => updateConfig({ captionBackgroundColor: color })}
                aria-label={t('live.background_color')}
              />
            </SettingsItem>

            <SettingsItem title={t('live.background_opacity')}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px', width: '212px' }}>
                <input
                  type="range"
                  min="0"
                  max="100"
                  step="1"
                  value={Math.round(captionBackgroundOpacity * 100)}
                  onChange={(e) =>
                    updateConfig({ captionBackgroundOpacity: Number(e.target.value) / 100 })
                  }
                  className="settings-slider"
                  style={{ flex: 1 }}
                />
                <span
                  style={{
                    width: '40px',
                    textAlign: 'right',
                    fontSize: '13px',
                    fontVariantNumeric: 'tabular-nums',
                  }}
                >
                  {Math.round(captionBackgroundOpacity * 100)}%
                </span>
              </div>
            </SettingsItem>
          </SettingsSection>
        </div>
      )}
    </SettingsTabContainer>
  );
}

export default SettingsSubtitleTab;
