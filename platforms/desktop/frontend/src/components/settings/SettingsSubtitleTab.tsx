import {
  Activity,
  Check,
  Copy,
  History,
  Keyboard,
  Layers,
  Plus,
  Search,
  SlidersHorizontal,
  Sparkles,
  Subtitles,
  Trash2,
  X,
} from 'lucide-react';
import type React from 'react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useVoiceTypingReadiness } from '../../hooks/useVoiceTypingReadiness';
import { DEFAULT_VOICE_TYPING_CONTEXT_RULES } from '../../services/voiceTyping/voiceTypingContext';
import {
  useCaptionConfig,
  useConfigStore,
  useSetConfig,
  useVoiceTypingConfig,
} from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import { useVoiceTypingHistoryStore } from '../../stores/voiceTypingHistoryStore';
import type { VoiceTypingRuntimeErrorSource } from '../../stores/voiceTypingRuntimeStore';
import type { VoiceTypingContextPreset } from '../../types/config';
import { logger } from '../../utils/logger';
import {
  addTermToYamlDictionary,
  serializeToYamlDictionary,
  VOICE_TYPING_SCOPE_ID,
} from '../../utils/yamlDictionaryParser';
import { ColorSwatchPicker } from '../ColorSwatchPicker';
import { Dropdown } from '../Dropdown';
import { SubtitleIcon } from '../Icons';
import { Switch } from '../Switch';
import { SettingsContextRulesSection } from './SettingsContextRulesSection';
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
    <>
      {/* 1. 快捷键与触发 (Interaction & Shortcuts) */}
      <SettingsSection
        title={t('settings.voice_typing_section_interaction', {
          defaultValue: 'Interaction & Shortcuts',
        })}
        icon={<Keyboard size={20} />}
        description={t('settings.voice_typing_section_interaction_desc', {
          defaultValue:
            'Configure global shortcut triggers, activation mode, and floating capsule placement',
        })}
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
          title={t('settings.voice_typing_quick_recall_shortcut', {
            defaultValue: 'Quick Recall Shortcut',
          })}
          hint={t('settings.voice_typing_quick_recall_shortcut_hint', {
            defaultValue: 'Open floating history to re-inject recent dictations',
          })}
        >
          <SettingsShortcutInput
            value={vtConfig.voiceTypingQuickRecallShortcut ?? 'Alt+Shift+H'}
            onChange={(val) => updateConfig({ voiceTypingQuickRecallShortcut: val })}
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
          title={t('settings.voice_typing_placement', { defaultValue: 'Placement Policy' })}
          hint={t('settings.voice_typing_placement_hint', {
            defaultValue: 'Choose where the floating capsule appears during dictation',
          })}
        >
          <div style={{ width: '220px' }}>
            <Dropdown
              id="vt-placement-select"
              value={vtConfig.voiceTypingPlacement || 'caret'}
              onChange={(val) =>
                updateConfig({
                  voiceTypingPlacement: val as 'caret' | 'bottom_center',
                })
              }
              options={[
                {
                  value: 'caret',
                  label: t('settings.voice_typing_placement_caret', {
                    defaultValue: 'Follow Caret (Caret Follower)',
                  }),
                },
                {
                  value: 'bottom_center',
                  label: t('settings.voice_typing_placement_bottom_center', {
                    defaultValue: 'Bottom Center (Dynamic Island)',
                  }),
                },
              ]}
            />
          </div>
        </SettingsItem>
      </SettingsSection>

      {/* 2. 识别与 AI 润色 (Processing & AI Polish) */}
      <SettingsSection
        title={t('settings.voice_typing_section_processing', {
          defaultValue: 'Processing & AI Polish',
        })}
        icon={<Sparkles size={20} />}
        description={t('settings.voice_typing_section_processing_desc', {
          defaultValue: 'Choose between instant raw speech-to-text or AI-powered smart polishing',
        })}
      >
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
            title={t('settings.voice_typing_polish_prompt', {
              defaultValue: 'Custom Polish Prompt',
            })}
            hint={t('settings.voice_typing_polish_prompt_hint', {
              defaultValue:
                'Leave blank to use the built-in fast colloquial-to-written prompt directive',
            })}
          >
            <textarea
              className="settings-input"
              rows={3}
              style={{
                width: '100%',
                maxWidth: '420px',
                resize: 'vertical',
                fontFamily: 'inherit',
                fontSize: '12px',
                lineHeight: 1.5,
              }}
              placeholder={t('settings.voice_typing_polish_prompt_hint', {
                defaultValue:
                  'Leave blank to use the built-in fast colloquial-to-written prompt directive',
              })}
              value={vtConfig.voiceTypingPolishPrompt ?? ''}
              onChange={(e) => updateConfig({ voiceTypingPolishPrompt: e.target.value })}
            />
          </SettingsItem>
        )}
      </SettingsSection>

      {/* 3. 排版与反馈 (Typography & Audio Feedback) */}
      <SettingsSection
        title={t('settings.voice_typing_section_typography_sound', {
          defaultValue: 'Typography & Audio Feedback',
        })}
        icon={<Subtitles size={20} />}
        description={t('settings.voice_typing_section_typography_sound_desc', {
          defaultValue: 'Fine-tune automatic CJK-Latin spacing and earcon sound effects',
        })}
      >
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
      </SettingsSection>

      {/* 4. 情境感知与规则 (Situational Context Awareness) */}
      <SettingsSection
        title={t('settings.voice_typing_section_context', {
          defaultValue: 'Situational Context Awareness',
        })}
        icon={<Layers size={20} />}
        description={t('settings.voice_typing_section_context_desc', {
          defaultValue:
            'Automatically adapt tone and prompt directives based on the active application',
        })}
      >
        <SettingsItem
          title={t('settings.voice_typing_context_awareness_enabled', {
            defaultValue: 'Application Context Awareness',
          })}
          hint={t('settings.voice_typing_context_awareness_enabled_hint', {
            defaultValue:
              'Automatically adapt tone, style, and formatting based on the active application and window context',
          })}
        >
          <Switch
            checked={vtConfig.voiceTypingContextAwarenessEnabled ?? true}
            onChange={(val) => updateConfig({ voiceTypingContextAwarenessEnabled: val })}
          />
        </SettingsItem>

        <SettingsItem
          title={t('settings.voice_typing_context_preset', {
            defaultValue: 'Situational Preset',
          })}
          hint={t('settings.voice_typing_context_preset_hint', {
            defaultValue:
              'Choose whether to auto-sense active applications or enforce a specific writing style',
          })}
        >
          <div style={{ width: '220px' }}>
            <Dropdown
              id="vt-context-preset-select"
              value={vtConfig.voiceTypingContextPreset || 'auto'}
              onChange={(val) =>
                updateConfig({
                  voiceTypingContextPreset: val as VoiceTypingContextPreset,
                })
              }
              options={[
                {
                  value: 'auto',
                  label: t('settings.voice_typing_context_preset_auto', {
                    defaultValue: 'Auto Detect (Recommended)',
                  }),
                },
                ...(vtConfig.voiceTypingContextRules ?? DEFAULT_VOICE_TYPING_CONTEXT_RULES).map(
                  (rule) => ({
                    value: rule.id,
                    label: `${rule.icon ?? ''} ${rule.name}`.trim(),
                  })
                ),
                {
                  value: 'general',
                  label: t('settings.voice_typing_context_preset_general', {
                    defaultValue: 'General Standard',
                  }),
                },
              ]}
            />
          </div>
        </SettingsItem>

        {vtConfig.voiceTypingContextAwarenessEnabled && <SettingsContextRulesSection />}
      </SettingsSection>

      {/* 5. 引擎就绪状态 (Engine Readiness) */}
      <SettingsSection
        title={t('settings.voice_typing_section_readiness', {
          defaultValue: 'Engine Readiness',
        })}
        icon={<Activity size={20} />}
        description={t('settings.voice_typing_section_readiness_desc', {
          defaultValue: 'Inspect speech recognition engine status and error diagnostics',
        })}
      >
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
    </>
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
  const [searchQuery, setSearchQuery] = useState('');

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
    const projects = useProjectStore.getState().projects;
    const voiceTypingName = t('settings.voice_typing', {
      defaultValue: 'Voice Typing',
    });

    // 1. Add to unified dictionary under voice typing scope
    const currentDict = config.dictionaryContent?.trim()
      ? config.dictionaryContent
      : serializeToYamlDictionary(
          config.hotwordSets || [],
          config.textReplacementSets || [],
          projects
        );

    const nextDict = addTermToYamlDictionary(
      currentDict,
      trimmed,
      { id: VOICE_TYPING_SCOPE_ID, name: voiceTypingName },
      projects
    );

    // 2. Also keep hotwordSets updated for backwards compatibility
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
        name: t('settings.voice_typing_hotwords_set_name', {
          defaultValue: 'Voice Typing Hotwords',
        }),
        enabled: true,
        rules: [{ id: `hw_${Date.now()}`, text: trimmed }],
      });
    }

    updateConfig({
      dictionaryContent: nextDict,
      hotwordSets: nextSets,
    });
    setAddedHotwordId(id);
    setTimeout(() => setAddedHotwordId((current) => (current === id ? null : current)), 1500);
  };

  const filteredItems = historyItems.filter((item) => {
    if (!searchQuery.trim()) return true;
    const q = searchQuery.toLowerCase();
    return (
      item.injectedText.toLowerCase().includes(q) ||
      Boolean(item.rawText?.toLowerCase().includes(q))
    );
  });

  return (
    <SettingsSection
      title={t('settings.voice_typing_history', { defaultValue: 'Dictation History' })}
      icon={<History size={20} />}
    >
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '12px',
          padding: '20px 24px 24px',
          background: 'var(--color-bg-primary)',
        }}
      >
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
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
              className="btn btn-secondary btn-sm"
              style={{ flexShrink: 0 }}
            >
              <Trash2 size={14} />
              <span>{t('common.clear', { defaultValue: 'Clear' })}</span>
            </button>
          )}
        </div>

        {historyItems.length > 0 && (
          <div style={{ position: 'relative' }}>
            <Search
              size={13}
              style={{
                position: 'absolute',
                left: '10px',
                top: '50%',
                transform: 'translateY(-50%)',
                color: 'var(--color-text-muted)',
                pointerEvents: 'none',
              }}
            />
            <input
              type="text"
              className="settings-input"
              style={{
                width: '100%',
                paddingLeft: '30px',
                paddingRight: searchQuery ? '28px' : '10px',
                fontSize: '12px',
                height: '32px',
              }}
              placeholder={t('settings.voice_typing_history_search_placeholder', {
                defaultValue: 'Search dictation history...',
              })}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
            {searchQuery && (
              <button
                type="button"
                onClick={() => setSearchQuery('')}
                style={{
                  position: 'absolute',
                  right: '8px',
                  top: '50%',
                  transform: 'translateY(-50%)',
                  background: 'none',
                  border: 'none',
                  color: 'var(--color-text-muted)',
                  cursor: 'pointer',
                  padding: '2px',
                  display: 'flex',
                  alignItems: 'center',
                }}
                title={t('common.clear', { defaultValue: 'Clear' })}
              >
                <X size={12} />
              </button>
            )}
          </div>
        )}

        {historyItems.length === 0 ? (
          <div
            data-testid="voice-typing-history-empty"
            style={{
              padding: '28px 16px',
              textAlign: 'center',
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              gap: '8px',
              color: 'var(--color-text-muted)',
              fontSize: '13px',
              borderRadius: 'var(--radius-md, 8px)',
              background: 'var(--color-bg-secondary)',
              border: '1px dashed var(--color-border)',
            }}
          >
            <History size={26} style={{ opacity: 0.4 }} />
            <span>
              {t('settings.voice_typing_history_empty', {
                defaultValue:
                  'No dictation history yet. Texts transcribed via voice typing will appear here.',
              })}
            </span>
          </div>
        ) : filteredItems.length === 0 ? (
          <div
            style={{
              padding: '20px 16px',
              textAlign: 'center',
              color: 'var(--color-text-muted)',
              fontSize: '12px',
              borderRadius: 'var(--radius-md, 8px)',
              background: 'var(--color-bg-secondary)',
              border: '1px solid var(--color-border)',
            }}
          >
            {t('common.no_results', { defaultValue: 'No matching records found.' })}
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
            {filteredItems.map((item) => (
              <div
                key={item.id}
                data-testid={`voice-typing-history-item-${item.id}`}
                style={{
                  display: 'flex',
                  alignItems: 'flex-start',
                  gap: '12px',
                  padding: '12px 14px',
                  borderRadius: 'var(--radius-md, 8px)',
                  background: 'var(--color-bg-secondary)',
                  border: '1px solid var(--color-border)',
                  transition: 'border-color 0.15s ease',
                }}
              >
                <div
                  style={{
                    flex: 1,
                    minWidth: 0,
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '6px',
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span
                      style={{
                        fontSize: '11px',
                        fontWeight: 600,
                        padding: '2px 7px',
                        borderRadius: '4px',
                        background:
                          item.mode === 'polish'
                            ? 'rgba(168, 85, 247, 0.12)'
                            : 'rgba(59, 130, 246, 0.12)',
                        color:
                          item.mode === 'polish'
                            ? 'var(--color-accent-purple, #a855f7)'
                            : 'var(--color-accent-blue, #3b82f6)',
                        border:
                          item.mode === 'polish'
                            ? '1px solid rgba(168, 85, 247, 0.25)'
                            : '1px solid rgba(59, 130, 246, 0.25)',
                      }}
                    >
                      {item.mode === 'polish'
                        ? t('settings.voice_typing_mode_badge_polish', {
                            defaultValue: 'AI Polish',
                          })
                        : t('settings.voice_typing_mode_badge_raw', {
                            defaultValue: 'Fast Dictation',
                          })}
                    </span>
                    <span style={{ fontSize: '11px', color: 'var(--color-text-muted)' }}>
                      {new Date(item.timestamp).toLocaleTimeString()}
                    </span>
                  </div>
                  <div
                    style={{
                      fontSize: '13px',
                      lineHeight: '1.6',
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
                        background: 'var(--color-bg-elevated)',
                        borderLeft: '2px solid var(--color-accent-purple, #a855f7)',
                        padding: '6px 10px',
                        borderRadius: '0 4px 4px 0',
                        lineHeight: '1.5',
                      }}
                    >
                      <span style={{ fontWeight: 500, opacity: 0.8 }}>
                        {t('settings.voice_typing_original_text', {
                          defaultValue: 'Original Draft',
                        })}
                        :
                      </span>{' '}
                      {item.rawText}
                    </div>
                  )}
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexShrink: 0 }}>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => void handleCopy(item.id, item.injectedText)}
                  >
                    {copiedId === item.id ? (
                      <Check size={14} color="#22c55e" />
                    ) : (
                      <Copy size={14} />
                    )}
                    <span>
                      {copiedId === item.id
                        ? t('common.copied', { defaultValue: 'Copied' })
                        : t('common.copy', { defaultValue: 'Copy' })}
                    </span>
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => handleAddToHotwords(item.id, item.injectedText)}
                  >
                    {addedHotwordId === item.id ? (
                      <Check size={14} color="#22c55e" />
                    ) : (
                      <Plus size={14} />
                    )}
                    <span>
                      {addedHotwordId === item.id
                        ? t('settings.voice_typing_hotword_added', { defaultValue: 'Added' })
                        : t('settings.voice_typing_add_hotword', {
                            defaultValue: 'Add to Hotwords',
                          })}
                    </span>
                  </button>
                  <button
                    type="button"
                    className="btn btn-icon btn-sm"
                    aria-label={t('common.delete', { defaultValue: 'Delete' })}
                    onClick={() => removeItem(item.id)}
                    style={{ color: 'var(--color-text-muted)' }}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
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
