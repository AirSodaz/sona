import type React from 'react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  DEFAULT_VOICE_TYPING_CONTEXT_RULES,
  getCurrentPlatform,
} from '../../services/voiceTyping/voiceTypingContext';
import { useSetConfig, useVoiceTypingConfig } from '../../stores/configStore';
import type { HostPlatform, VoiceTypingContextRule } from '../../types/config';
import { Modal } from '../Modal';
import { Switch } from '../Switch';
import { Button, IconButton } from '../ui/Button';

const PRESET_ICONS = ['💻', '💬', '📄', '🎓', '📧', '✍️', '⚡', '🛠️', '🎨', '🌐', '📊', '🔍'];

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

  const openAddRule = () => {
    const newDraft: RuleDraft = {
      id: `custom-${Date.now()}`,
      name: '',
      icon: '✨',
      badgeColor: '#5c4d43',
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
    setIsModalOpen(true);
  };

  const closeModal = () => {
    setIsModalOpen(false);
    setDraft(null);
    setEditingRuleId(null);
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

  const handleRemoveApp = (appName: string) => {
    if (!draft) return;
    const currentList = draft.appsByPlatform[currentPlatform] ?? [];
    setDraft({
      ...draft,
      appsByPlatform: {
        ...draft.appsByPlatform,
        [currentPlatform]: currentList.filter((item) => item !== appName),
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
      titlePatterns: draft.titlePatterns.filter((p) => p !== pattern),
    });
  };

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '14px',
        padding: '20px 24px 24px',
        background: 'var(--color-bg-primary)',
      }}
    >
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
        }}
      >
        <div>
          <span
            style={{
              fontWeight: 600,
              fontSize: '13px',
              color: 'var(--color-text-primary)',
            }}
          >
            {t('settings.voice_typing_rules_section_title', {
              defaultValue: 'Situational Context Rules',
            })}
          </span>
          <p
            style={{
              fontSize: '12px',
              color: 'var(--color-text-muted)',
              margin: '2px 0 0 0',
            }}
          >
            {t('settings.voice_typing_rules_section_hint', {
              defaultValue:
                'Customize which applications trigger specific writing styles on this device',
            })}
          </p>
        </div>
        <Button
          type="button"
          data-testid="add-context-rule-btn"
          tier="secondary"
          size="sm"
          leftIcon={<span>+</span>}
          onClick={openAddRule}
          style={{ flexShrink: 0 }}
        >
          {t('settings.voice_typing_add_rule_button', {
            defaultValue: 'Add Situation',
          })}
        </Button>
      </div>

      <div
        data-testid="voice-typing-rules-list"
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '8px',
          background: 'var(--color-bg-secondary)',
          borderRadius: 'var(--radius-lg, 12px)',
          padding: '10px',
          border: '1px solid var(--color-border)',
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
                borderRadius: 'var(--radius-md, 8px)',
                background: 'var(--color-bg-primary)',
                border: '1px solid var(--color-border)',
                gap: '14px',
                transition: 'border-color 0.15s ease',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
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
                    background: 'var(--color-bg-elevated)',
                    border: '1px solid var(--color-border)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: '18px',
                    flexShrink: 0,
                  }}
                >
                  {rule.icon ?? '✨'}
                </div>
                <div style={{ minWidth: 0 }}>
                  <div
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px',
                      flexWrap: 'wrap',
                    }}
                  >
                    <span
                      style={{
                        fontWeight: 600,
                        fontSize: '13px',
                        color: 'var(--color-text-primary)',
                      }}
                    >
                      {rule.name}
                    </span>
                    {rule.isBuiltin && (
                      <span
                        style={{
                          fontSize: '10px',
                          fontWeight: 500,
                          padding: '1px 5px',
                          borderRadius: '4px',
                          background: 'var(--color-bg-tertiary)',
                          color: 'var(--color-text-muted)',
                          border: '1px solid var(--color-border)',
                        }}
                      >
                        {t('settings.voice_typing_rule_builtin_badge', {
                          defaultValue: 'Built-in',
                        })}
                      </span>
                    )}
                    {rule.stripTrailingPunctuation && (
                      <span
                        style={{
                          fontSize: '10px',
                          fontWeight: 500,
                          padding: '1px 5px',
                          borderRadius: '4px',
                          background: 'var(--color-bg-hover)',
                          color: 'var(--color-text-secondary)',
                          border: '1px solid var(--color-border)',
                        }}
                      >
                        {t('settings.voice_typing_rule_strip_active', {
                          defaultValue: 'No ending period',
                        })}
                      </span>
                    )}
                  </div>
                  <div
                    style={{
                      fontSize: '11px',
                      color: 'var(--color-text-muted)',
                      marginTop: '2px',
                    }}
                  >
                    {t('settings.voice_typing_rule_apps_summary', {
                      count: platformApps.length,
                      platform: platformName,
                      defaultValue: `${platformApps.length} apps mapped on ${platformName}`,
                    })}
                  </div>
                </div>
              </div>

              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexShrink: 0 }}>
                <Switch
                  checked={rule.enabled}
                  onChange={(checked) => handleToggleRule(rule.id, checked)}
                />
                <Button
                  data-testid={`rule-edit-btn-${rule.id}`}
                  tier="secondary"
                  size="sm"
                  onClick={() => openEditRule(rule)}
                >
                  {t('common.edit', { defaultValue: 'Edit' })}
                </Button>
                {rule.isBuiltin ? (
                  <Button
                    data-testid={`rule-reset-btn-${rule.id}`}
                    tier="secondary"
                    size="sm"
                    onClick={() => handleResetRule(rule.id)}
                  >
                    {t('common.reset', { defaultValue: 'Reset' })}
                  </Button>
                ) : (
                  <Button
                    data-testid={`rule-delete-btn-${rule.id}`}
                    tier="ghost"
                    tone="danger"
                    size="sm"
                    onClick={() => handleDeleteRule(rule.id)}
                  >
                    {t('common.delete', { defaultValue: 'Delete' })}
                  </Button>
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
              <Button tier="secondary" onClick={closeModal}>
                {t('common.cancel', { defaultValue: 'Cancel' })}
              </Button>
              <Button tier="primary" onClick={handleSaveDraft} disabled={!draft.name.trim()}>
                {t('common.save', { defaultValue: 'Save' })}
              </Button>
            </div>
          }
        >
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
            {/* Rule Name & Icon */}
            <div>
              <div style={{ display: 'flex', gap: '12px', alignItems: 'flex-start' }}>
                <div style={{ width: '84px', flexShrink: 0 }}>
                  <label
                    className="settings-label"
                    style={{
                      fontSize: '12px',
                      fontWeight: 600,
                      display: 'block',
                      marginBottom: '4px',
                    }}
                  >
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
                  <label
                    className="settings-label"
                    style={{
                      fontSize: '12px',
                      fontWeight: 600,
                      display: 'block',
                      marginBottom: '4px',
                    }}
                  >
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

              {/* Preset Icon Shortcuts */}
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px', marginTop: '8px' }}>
                {PRESET_ICONS.map((emoji) => (
                  <button
                    key={emoji}
                    type="button"
                    onClick={() => setDraft({ ...draft, icon: emoji })}
                    style={{
                      width: '30px',
                      height: '30px',
                      borderRadius: '6px',
                      border:
                        draft.icon === emoji
                          ? '1px solid var(--color-brand)'
                          : '1px solid var(--color-border)',
                      background:
                        draft.icon === emoji
                          ? 'var(--color-bg-elevated)'
                          : 'var(--color-bg-secondary)',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      fontSize: '14px',
                      cursor: 'pointer',
                      transition: 'all 0.15s ease',
                    }}
                    aria-label={emoji}
                  >
                    {emoji}
                  </button>
                ))}
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
                <span style={{ fontSize: '11px', color: 'var(--color-text-muted)' }}>
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
                  minHeight: '38px',
                  padding: '6px 8px',
                  background: 'var(--color-bg-secondary)',
                  borderRadius: '6px',
                  border: '1px solid var(--color-border)',
                  marginBottom: '8px',
                  alignItems: 'center',
                }}
              >
                {(draft.appsByPlatform[currentPlatform] ?? []).length === 0 ? (
                  <span
                    style={{
                      fontSize: '12px',
                      color: 'var(--color-text-muted)',
                      fontStyle: 'italic',
                      padding: '2px 4px',
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
                        gap: '6px',
                        background: 'var(--color-bg-primary)',
                        border: '1px solid var(--color-border)',
                        padding: '3px 8px',
                        borderRadius: '6px',
                        fontSize: '12px',
                        fontFamily: 'var(--font-mono, monospace)',
                        color: 'var(--color-text-primary)',
                      }}
                    >
                      <span>{app}</span>
                      <IconButton
                        tier="ghost"
                        tone="danger"
                        size="xs"
                        icon={<span aria-hidden="true">×</span>}
                        onClick={() => handleRemoveApp(app)}
                        aria-label={t('common.remove', { defaultValue: 'Remove' })}
                      />
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
                <Button
                  data-testid="add-app-btn"
                  tier="secondary"
                  size="sm"
                  onClick={handleAddApp}
                  disabled={!appInput.trim()}
                >
                  {t('common.add', { defaultValue: 'Add' })}
                </Button>
              </div>
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
                  minHeight: '38px',
                  padding: '6px 8px',
                  background: 'var(--color-bg-secondary)',
                  borderRadius: '6px',
                  border: '1px solid var(--color-border)',
                  marginBottom: '8px',
                  alignItems: 'center',
                }}
              >
                {draft.titlePatterns.length === 0 ? (
                  <span
                    style={{
                      fontSize: '12px',
                      color: 'var(--color-text-muted)',
                      fontStyle: 'italic',
                      padding: '2px 4px',
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
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: '6px',
                        background: 'var(--color-bg-primary)',
                        border: '1px solid var(--color-border)',
                        padding: '3px 8px',
                        borderRadius: '6px',
                        fontSize: '12px',
                        color: 'var(--color-text-primary)',
                      }}
                    >
                      <span>{pattern}</span>
                      <IconButton
                        tier="ghost"
                        tone="danger"
                        size="xs"
                        icon={<span aria-hidden="true">×</span>}
                        onClick={() => handleRemoveTitlePattern(pattern)}
                        aria-label={t('common.remove', { defaultValue: 'Remove' })}
                      />
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
                <Button
                  data-testid="add-title-pattern-btn"
                  tier="secondary"
                  size="sm"
                  onClick={handleAddTitlePattern}
                  disabled={!titleInput.trim()}
                >
                  {t('common.add', { defaultValue: 'Add' })}
                </Button>
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
                style={{
                  width: '100%',
                  resize: 'vertical',
                  fontFamily: 'var(--font-mono, monospace)',
                  fontSize: '12px',
                  lineHeight: 1.6,
                  padding: '8px 10px',
                }}
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
                background: 'var(--color-bg-secondary)',
                border: '1px solid var(--color-border)',
                borderRadius: '8px',
                padding: '12px 14px',
              }}
            >
              <div>
                <span
                  style={{ fontSize: '13px', fontWeight: 500, color: 'var(--color-text-primary)' }}
                >
                  {t('settings.voice_typing_strip_trailing_punct', {
                    defaultValue: 'Strip Trailing Punctuation in Raw Mode',
                  })}
                </span>
                <p
                  style={{
                    fontSize: '11px',
                    color: 'var(--color-text-muted)',
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
