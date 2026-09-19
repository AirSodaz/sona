import type React from 'react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getForegroundWindowInfo } from '../../services/tauri/system';
import {
  DEFAULT_VOICE_TYPING_CONTEXT_RULES,
  getCurrentPlatform,
} from '../../services/voiceTyping/voiceTypingContext';
import { useSetConfig, useVoiceTypingConfig } from '../../stores/configStore';
import type { HostPlatform, VoiceTypingContextRule } from '../../types/config';
import { extractErrorMessage } from '../../utils/errorUtils';
import { logger } from '../../utils/logger';
import { Modal } from '../Modal';
import { Switch } from '../Switch';

interface RuleDraft {
  id: string;
  name: string;
  icon: string;
  badgeColor?: string;
  appsByPlatform: {
    windows: string[];
    macos: string[];
    linux: string[];
  };
  titlePatterns: string[];
  promptDirective: string;
  stripTrailingPunctuation: boolean;
  isBuiltin: boolean;
  enabled: boolean;
}

function getPlatformDisplayName(platform: HostPlatform): string {
  switch (platform) {
    case 'windows':
      return 'Windows';
    case 'macos':
      return 'macOS';
    case 'linux':
      return 'Linux';
  }
}

export function SettingsContextRulesSection(): React.JSX.Element {
  const { t } = useTranslation();
  const vtConfig = useVoiceTypingConfig();
  const updateConfig = useSetConfig();

  const rules = vtConfig.voiceTypingContextRules ?? DEFAULT_VOICE_TYPING_CONTEXT_RULES;
  const currentPlatform = getCurrentPlatform();
  const platformName = getPlatformDisplayName(currentPlatform);

  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingRuleId, setEditingRuleId] = useState<string | null>(null);
  const [draft, setDraft] = useState<RuleDraft | null>(null);

  const [appInput, setAppInput] = useState('');
  const [titleInput, setTitleInput] = useState('');
  const [isCapturing, setIsCapturing] = useState(false);
  const [captureFeedback, setCaptureFeedback] = useState<string | null>(null);

  const openAddRule = () => {
    const newDraft: RuleDraft = {
      id: `custom-${Date.now()}`,
      name: '',
      icon: '✨',
      badgeColor: '#6366f1',
      appsByPlatform: {
        windows: [],
        macos: [],
        linux: [],
      },
      titlePatterns: [],
      promptDirective: '',
      stripTrailingPunctuation: false,
      isBuiltin: false,
      enabled: true,
    };
    setDraft(newDraft);
    setEditingRuleId(null);
    setAppInput('');
    setTitleInput('');
    setCaptureFeedback(null);
    setIsModalOpen(true);
  };

  const openEditRule = (rule: VoiceTypingContextRule) => {
    const editDraft: RuleDraft = {
      ...rule,
      icon: rule.icon ?? '✨',
      appsByPlatform: {
        windows: [...(rule.appsByPlatform?.windows ?? [])],
        macos: [...(rule.appsByPlatform?.macos ?? [])],
        linux: [...(rule.appsByPlatform?.linux ?? [])],
      },
      titlePatterns: [...(rule.titlePatterns ?? [])],
    };
    setDraft(editDraft);
    setEditingRuleId(rule.id);
    setAppInput('');
    setTitleInput('');
    setCaptureFeedback(null);
    setIsModalOpen(true);
  };

  const closeModal = () => {
    setIsModalOpen(false);
    setDraft(null);
    setEditingRuleId(null);
    setCaptureFeedback(null);
  };

  const handleToggleRule = (id: string, enabled: boolean) => {
    const nextRules = rules.map((r) => (r.id === id ? { ...r, enabled } : r));
    updateConfig({ voiceTypingContextRules: nextRules });
  };

  const handleDeleteRule = (id: string) => {
    const nextRules = rules.filter((r) => r.id !== id);
    updateConfig({ voiceTypingContextRules: nextRules });
  };

  const handleResetRule = (id: string) => {
    const defaultRule = DEFAULT_VOICE_TYPING_CONTEXT_RULES.find((r) => r.id === id);
    if (!defaultRule) return;
    const nextRules = rules.map((r) => (r.id === id ? { ...defaultRule } : r));
    updateConfig({ voiceTypingContextRules: nextRules });
  };

  const handleSaveDraft = () => {
    if (!draft?.name.trim()) return;

    const trimmedDraft: VoiceTypingContextRule = {
      ...draft,
      name: draft.name.trim(),
      icon: draft.icon.trim() || '✨',
      promptDirective: draft.promptDirective.trim(),
    };

    let nextRules: VoiceTypingContextRule[];
    if (editingRuleId) {
      nextRules = rules.map((r) => (r.id === editingRuleId ? trimmedDraft : r));
    } else {
      nextRules = [...rules, trimmedDraft];
    }

    updateConfig({ voiceTypingContextRules: nextRules });
    closeModal();
  };

  const handleAddApp = () => {
    if (!draft || !appInput.trim()) return;
    const clean = appInput.trim();
    const currentList = draft.appsByPlatform[currentPlatform] ?? [];
    if (!currentList.includes(clean)) {
      setDraft({
        ...draft,
        appsByPlatform: {
          ...draft.appsByPlatform,
          [currentPlatform]: [...currentList, clean],
        },
      });
    }
    setAppInput('');
  };

  const handleRemoveApp = (app: string) => {
    if (!draft) return;
    const currentList = draft.appsByPlatform[currentPlatform] ?? [];
    setDraft({
      ...draft,
      appsByPlatform: {
        ...draft.appsByPlatform,
        [currentPlatform]: currentList.filter((item) => item !== app),
      },
    });
  };

  const handleAddTitlePattern = () => {
    if (!draft || !titleInput.trim()) return;
    const clean = titleInput.trim();
    if (!draft.titlePatterns.includes(clean)) {
      setDraft({
        ...draft,
        titlePatterns: [...draft.titlePatterns, clean],
      });
    }
    setTitleInput('');
  };

  const handleRemoveTitlePattern = (pattern: string) => {
    if (!draft) return;
    setDraft({
      ...draft,
      titlePatterns: draft.titlePatterns.filter((item) => item !== pattern),
    });
  };

  const handleCaptureCurrentApp = async () => {
    if (!draft) return;
    setIsCapturing(true);
    setCaptureFeedback(null);
    try {
      const info = await getForegroundWindowInfo();
      if (info?.appName) {
        const clean = info.appName.trim();
        const currentList = draft.appsByPlatform[currentPlatform] ?? [];
        if (!currentList.includes(clean)) {
          setDraft({
            ...draft,
            appsByPlatform: {
              ...draft.appsByPlatform,
              [currentPlatform]: [...currentList, clean],
            },
          });
          setCaptureFeedback(
            t('settings.voice_typing_captured_success', {
              app: clean,
              defaultValue: `Captured: ${clean}`,
            })
          );
        } else {
          setCaptureFeedback(
            t('settings.voice_typing_captured_already_exists', {
              app: clean,
              defaultValue: `${clean} is already in the list`,
            })
          );
        }
      } else {
        setCaptureFeedback(
          t('settings.voice_typing_captured_none', {
            defaultValue: 'No active application window detected',
          })
        );
      }
    } catch (err) {
      logger.error('[SettingsContextRules] Failed to capture foreground window:', err);
      setCaptureFeedback(extractErrorMessage(err));
    } finally {
      setIsCapturing(false);
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginTop: '8px',
        }}
      >
        <div>
          <span style={{ fontWeight: 600, fontSize: '14px' }}>
            {t('settings.voice_typing_rules_section_title', {
              defaultValue: 'Situational Context Rules',
            })}
          </span>
          <p
            style={{
              fontSize: '12px',
              color: 'var(--color-text-secondary, #888)',
              margin: '2px 0 0 0',
            }}
          >
            {t('settings.voice_typing_rules_section_hint', {
              defaultValue:
                'Customize which applications trigger specific writing styles on this device',
            })}
          </p>
        </div>
        <button
          type="button"
          data-testid="add-context-rule-btn"
          className="btn btn-secondary btn-sm"
          onClick={openAddRule}
          style={{ display: 'flex', alignItems: 'center', gap: '4px' }}
        >
          <span>+</span>
          <span>
            {t('settings.voice_typing_add_rule_button', {
              defaultValue: 'Add Situation',
            })}
          </span>
        </button>
      </div>

      <div
        data-testid="voice-typing-rules-list"
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '8px',
          background: 'var(--color-bg-secondary, rgba(255, 255, 255, 0.04))',
          borderRadius: '8px',
          padding: '8px',
          border: '1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.08))',
        }}
      >
        {rules.map((rule) => {
          const platformApps = rule.appsByPlatform?.[currentPlatform] ?? [];
          return (
            <div
              key={rule.id}
              data-testid={`voice-typing-rule-card-${rule.id}`}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '12px 14px',
                borderRadius: '8px',
                background: 'var(--color-bg-primary, rgba(0, 0, 0, 0.2))',
                border: '1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.06))',
                gap: '14px',
                transition: 'border-color 0.15s ease',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  alignItems: 'flex-start',
                  gap: '12px',
                  minWidth: 0,
                  flex: 1,
                }}
              >
                <div
                  style={{
                    width: '38px',
                    height: '38px',
                    borderRadius: '8px',
                    background: 'var(--color-bg-elevated, rgba(255, 255, 255, 0.06))',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: '20px',
                    flexShrink: 0,
                    boxShadow: 'inset 0 0 0 1px rgba(255, 255, 255, 0.06)',
                  }}
                >
                  {rule.icon ?? '✨'}
                </div>
                <div style={{ minWidth: 0 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span style={{ fontWeight: 600, fontSize: '13px' }}>{rule.name}</span>
                    {rule.isBuiltin && (
                      <span
                        style={{
                          fontSize: '10px',
                          padding: '1px 5px',
                          borderRadius: '3px',
                          background: 'rgba(255, 255, 255, 0.08)',
                          color: 'var(--color-text-secondary, #aaa)',
                        }}
                      >
                        {t('settings.voice_typing_rule_builtin_badge', {
                          defaultValue: 'Built-in',
                        })}
                      </span>
                    )}
                  </div>
                  <div
                    style={{
                      fontSize: '11px',
                      color: 'var(--color-text-secondary, #888)',
                      marginTop: '2px',
                    }}
                  >
                    {t('settings.voice_typing_rule_apps_summary', {
                      count: platformApps.length,
                      platform: platformName,
                      defaultValue: `${platformApps.length} apps mapped on ${platformName}`,
                    })}
                    {rule.stripTrailingPunctuation
                      ? ` · ${t('settings.voice_typing_rule_strip_active', { defaultValue: 'No ending period' })}`
                      : ''}
                  </div>

                  {/* Visual software tags preview */}
                  <div
                    style={{
                      display: 'flex',
                      flexWrap: 'wrap',
                      alignItems: 'center',
                      gap: '4px',
                      marginTop: '6px',
                    }}
                  >
                    {platformApps.length > 0 ? (
                      <>
                        {platformApps.slice(0, 4).map((app) => (
                          <span
                            key={app}
                            style={{
                              fontSize: '11px',
                              fontFamily: 'monospace',
                              background: 'var(--color-bg-elevated, rgba(255, 255, 255, 0.06))',
                              color: 'var(--color-text-secondary, #aaa)',
                              padding: '1px 6px',
                              borderRadius: '4px',
                              border:
                                '1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.06))',
                            }}
                          >
                            {app}
                          </span>
                        ))}
                        {platformApps.length > 4 && (
                          <span
                            style={{
                              fontSize: '10px',
                              color: 'var(--color-text-muted, #777)',
                              padding: '0 2px',
                            }}
                          >
                            +{platformApps.length - 4}
                          </span>
                        )}
                      </>
                    ) : (
                      <span
                        style={{
                          fontSize: '11px',
                          color: 'var(--color-text-muted, #777)',
                          fontStyle: 'italic',
                        }}
                      >
                        {t('settings.voice_typing_no_apps_mapped', {
                          defaultValue: 'No applications mapped yet for this OS.',
                        })}
                      </span>
                    )}
                  </div>
                </div>
              </div>

              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexShrink: 0 }}>
                <Switch
                  checked={rule.enabled}
                  onChange={(checked) => handleToggleRule(rule.id, checked)}
                />
                <button
                  type="button"
                  data-testid={`rule-edit-btn-${rule.id}`}
                  className="btn btn-secondary btn-sm"
                  onClick={() => openEditRule(rule)}
                >
                  {t('common.edit', { defaultValue: 'Edit' })}
                </button>
                {rule.isBuiltin ? (
                  <button
                    type="button"
                    data-testid={`rule-reset-btn-${rule.id}`}
                    className="btn btn-secondary btn-sm"
                    title={t('common.reset', { defaultValue: 'Reset' })}
                    onClick={() => handleResetRule(rule.id)}
                  >
                    {t('common.reset', { defaultValue: 'Reset' })}
                  </button>
                ) : (
                  <button
                    type="button"
                    data-testid={`rule-delete-btn-${rule.id}`}
                    className="btn btn-secondary btn-sm"
                    title={t('common.delete', { defaultValue: 'Delete' })}
                    onClick={() => handleDeleteRule(rule.id)}
                    style={{ color: 'var(--color-danger, #ef4444)' }}
                  >
                    {t('common.delete', { defaultValue: 'Delete' })}
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {isModalOpen && draft && (
        <Modal
          isOpen={isModalOpen}
          onClose={closeModal}
          title={
            editingRuleId
              ? t('settings.voice_typing_edit_rule_title', {
                  name: draft.name,
                  defaultValue: `Edit Situation: ${draft.name}`,
                })
              : t('settings.voice_typing_add_rule_title', {
                  defaultValue: 'Add Custom Situation',
                })
          }
          size="md"
          footer={
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
              <button type="button" className="btn btn-secondary" onClick={closeModal}>
                {t('common.cancel', { defaultValue: 'Cancel' })}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={handleSaveDraft}
                disabled={!draft.name.trim()}
              >
                {t('common.save', { defaultValue: 'Save' })}
              </button>
            </div>
          }
        >
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
            {/* Rule Name & Icon */}
            <div style={{ display: 'flex', gap: '12px' }}>
              <div style={{ width: '80px' }}>
                <label className="settings-label" style={{ fontSize: '12px', fontWeight: 600 }}>
                  {t('settings.voice_typing_rule_icon', { defaultValue: 'Icon' })}
                </label>
                <input
                  type="text"
                  className="settings-input"
                  style={{ width: '100%', textAlign: 'center', fontSize: '18px' }}
                  value={draft.icon}
                  onChange={(e) => setDraft({ ...draft, icon: e.target.value })}
                  maxLength={4}
                />
              </div>
              <div style={{ flex: 1 }}>
                <label className="settings-label" style={{ fontSize: '12px', fontWeight: 600 }}>
                  {t('settings.voice_typing_rule_name', { defaultValue: 'Situation Name' })}
                </label>
                <input
                  type="text"
                  className="settings-input"
                  style={{ width: '100%' }}
                  placeholder={t('settings.voice_typing_rule_name_placeholder', {
                    defaultValue: 'e.g. Email / Customer Service',
                  })}
                  value={draft.name}
                  onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                />
              </div>
            </div>

            {/* Current Platform App Tag List */}
            <div>
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'baseline',
                  marginBottom: '4px',
                }}
              >
                <label className="settings-label" style={{ fontSize: '12px', fontWeight: 600 }}>
                  {t('settings.voice_typing_platform_apps_title', {
                    platform: platformName,
                    defaultValue: `${platformName} Applications`,
                  })}
                </label>
                <span style={{ fontSize: '11px', color: 'var(--color-text-secondary, #888)' }}>
                  {t('settings.voice_typing_platform_apps_only_current', {
                    defaultValue: 'Configured only for your current operating system',
                  })}
                </span>
              </div>

              <div
                style={{
                  display: 'flex',
                  flexWrap: 'wrap',
                  gap: '6px',
                  minHeight: '36px',
                  padding: '6px 8px',
                  background: 'var(--color-bg-secondary, rgba(255, 255, 255, 0.04))',
                  borderRadius: '6px',
                  border: '1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.08))',
                  marginBottom: '8px',
                }}
              >
                {(draft.appsByPlatform[currentPlatform] ?? []).length === 0 ? (
                  <span
                    style={{
                      fontSize: '12px',
                      color: 'var(--color-text-secondary, #888)',
                      alignSelf: 'center',
                    }}
                  >
                    {t('settings.voice_typing_no_apps_mapped', {
                      defaultValue: 'No applications mapped yet for this OS.',
                    })}
                  </span>
                ) : (
                  (draft.appsByPlatform[currentPlatform] ?? []).map((app) => (
                    <span
                      key={app}
                      data-testid={`app-chip-${app}`}
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: '4px',
                        background: 'rgba(255, 255, 255, 0.1)',
                        padding: '2px 8px',
                        borderRadius: '4px',
                        fontSize: '12px',
                        fontFamily: 'monospace',
                      }}
                    >
                      <span>{app}</span>
                      <button
                        type="button"
                        onClick={() => handleRemoveApp(app)}
                        style={{
                          background: 'none',
                          border: 'none',
                          color: 'var(--color-text-secondary, #aaa)',
                          cursor: 'pointer',
                          padding: '0 2px',
                          fontSize: '14px',
                          lineHeight: 1,
                        }}
                        title={t('common.remove', { defaultValue: 'Remove' })}
                      >
                        ×
                      </button>
                    </span>
                  ))
                )}
              </div>

              <div style={{ display: 'flex', gap: '8px' }}>
                <input
                  type="text"
                  className="settings-input"
                  style={{ flex: 1 }}
                  placeholder={
                    currentPlatform === 'windows'
                      ? t('settings.voice_typing_app_input_placeholder_win', {
                          defaultValue: 'e.g. code.exe, slack.exe',
                        })
                      : t('settings.voice_typing_app_input_placeholder_unix', {
                          defaultValue: 'e.g. Code, Terminal, Slack',
                        })
                  }
                  value={appInput}
                  onChange={(e) => setAppInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      handleAddApp();
                    }
                  }}
                />
                <button
                  type="button"
                  data-testid="add-app-btn"
                  className="btn btn-secondary btn-sm"
                  onClick={handleAddApp}
                  disabled={!appInput.trim()}
                >
                  {t('common.add', { defaultValue: 'Add' })}
                </button>
                <button
                  type="button"
                  data-testid="capture-active-app-btn"
                  className="btn btn-secondary btn-sm"
                  onClick={handleCaptureCurrentApp}
                  disabled={isCapturing}
                  title={t('settings.voice_typing_capture_app_hint', {
                    defaultValue: 'Capture the active application currently in foreground',
                  })}
                  style={{ display: 'flex', alignItems: 'center', gap: '4px' }}
                >
                  <span>🔍</span>
                  <span>
                    {isCapturing
                      ? t('common.capturing', { defaultValue: 'Capturing...' })
                      : t('settings.voice_typing_capture_app_button', {
                          defaultValue: 'Capture Active App',
                        })}
                  </span>
                </button>
              </div>

              {captureFeedback && (
                <div
                  style={{
                    fontSize: '11px',
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: '4px',
                    padding: '3px 8px',
                    borderRadius: '6px',
                    background: 'rgba(99, 102, 241, 0.12)',
                    border: '1px solid rgba(99, 102, 241, 0.25)',
                    color: 'var(--color-accent-primary, #818cf8)',
                    marginTop: '6px',
                  }}
                >
                  <span>✓</span>
                  <span>{captureFeedback}</span>
                </div>
              )}
            </div>

            {/* Window Title Patterns */}
            <div>
              <label
                className="settings-label"
                style={{ fontSize: '12px', fontWeight: 600, display: 'block', marginBottom: '4px' }}
              >
                {t('settings.voice_typing_title_patterns_title', {
                  defaultValue: 'Window Title Keywords (Optional)',
                })}
              </label>

              <div
                style={{
                  display: 'flex',
                  flexWrap: 'wrap',
                  gap: '6px',
                  minHeight: '36px',
                  padding: '6px 8px',
                  background: 'var(--color-bg-secondary, rgba(255, 255, 255, 0.04))',
                  borderRadius: '6px',
                  border: '1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.08))',
                  marginBottom: '8px',
                }}
              >
                {draft.titlePatterns.length === 0 ? (
                  <span
                    style={{
                      fontSize: '12px',
                      color: 'var(--color-text-secondary, #888)',
                      alignSelf: 'center',
                    }}
                  >
                    {t('settings.voice_typing_no_title_patterns', {
                      defaultValue:
                        'Matches browser tabs or windows by keyword (e.g. github, slack)',
                    })}
                  </span>
                ) : (
                  draft.titlePatterns.map((pattern) => (
                    <span
                      key={pattern}
                      data-testid={`pattern-chip-${pattern}`}
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: '4px',
                        background: 'rgba(255, 255, 255, 0.1)',
                        padding: '2px 8px',
                        borderRadius: '4px',
                        fontSize: '12px',
                      }}
                    >
                      <span>{pattern}</span>
                      <button
                        type="button"
                        onClick={() => handleRemoveTitlePattern(pattern)}
                        style={{
                          background: 'none',
                          border: 'none',
                          color: 'var(--color-text-secondary, #aaa)',
                          cursor: 'pointer',
                          padding: '0 2px',
                          fontSize: '14px',
                          lineHeight: 1,
                        }}
                        title={t('common.remove', { defaultValue: 'Remove' })}
                      >
                        ×
                      </button>
                    </span>
                  ))
                )}
              </div>

              <div style={{ display: 'flex', gap: '8px' }}>
                <input
                  type="text"
                  className="settings-input"
                  style={{ flex: 1 }}
                  placeholder={t('settings.voice_typing_pattern_input_placeholder', {
                    defaultValue: 'e.g. github.com, stackoverflow',
                  })}
                  value={titleInput}
                  onChange={(e) => setTitleInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      handleAddTitlePattern();
                    }
                  }}
                />
                <button
                  type="button"
                  data-testid="add-title-pattern-btn"
                  className="btn btn-secondary btn-sm"
                  onClick={handleAddTitlePattern}
                  disabled={!titleInput.trim()}
                >
                  {t('common.add', { defaultValue: 'Add' })}
                </button>
              </div>
            </div>

            {/* Prompt Directive */}
            <div>
              <label
                className="settings-label"
                style={{ fontSize: '12px', fontWeight: 600, display: 'block', marginBottom: '4px' }}
              >
                {t('settings.voice_typing_prompt_directive_title', {
                  defaultValue: 'Dedicated Polish Directive',
                })}
              </label>
              <textarea
                className="settings-input"
                rows={3}
                style={{ width: '100%', resize: 'vertical' }}
                placeholder={t('settings.voice_typing_prompt_directive_placeholder', {
                  defaultValue:
                    'Directives appended to the LLM system prompt when this situation matches',
                })}
                value={draft.promptDirective}
                onChange={(e) => setDraft({ ...draft, promptDirective: e.target.value })}
              />
            </div>

            {/* Behavior Options */}
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                background: 'var(--color-bg-secondary, rgba(255, 255, 255, 0.03))',
                border: '1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.06))',
                borderRadius: '8px',
                padding: '12px 14px',
              }}
            >
              <div>
                <span style={{ fontSize: '13px', fontWeight: 500 }}>
                  {t('settings.voice_typing_strip_trailing_punct', {
                    defaultValue: 'Strip Trailing Punctuation in Raw Mode',
                  })}
                </span>
                <p
                  style={{
                    fontSize: '11px',
                    color: 'var(--color-text-secondary, #888)',
                    margin: '2px 0 0 0',
                  }}
                >
                  {t('settings.voice_typing_strip_trailing_punct_hint', {
                    defaultValue:
                      'Automatically omit periods or full stops when dictating into chats or terminals',
                  })}
                </p>
              </div>
              <Switch
                checked={draft.stripTrailingPunctuation}
                onChange={(checked) => setDraft({ ...draft, stripTrailingPunctuation: checked })}
              />
            </div>
          </div>
        </Modal>
      )}
    </div>
  );
}
