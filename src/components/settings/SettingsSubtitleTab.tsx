import React from 'react';
import { useTranslation } from 'react-i18next';
import { Check, Keyboard, Subtitles, SlidersHorizontal, X } from 'lucide-react';
import { useVoiceTypingReadiness } from '../../hooks/useVoiceTypingReadiness';
import { SubtitleIcon } from '../Icons';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { useCaptionConfig, useSetConfig, useVoiceTypingConfig } from '../../stores/configStore';
import { VoiceTypingRuntimeErrorSource } from '../../stores/voiceTypingRuntimeStore';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';
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
    source: VoiceTypingRuntimeErrorSource | null,
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
                        onChange={(val) =>
                            updateConfig({ voiceTypingMode: val as 'hold' | 'toggle' })
                        }
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

export function SettingsSubtitleTab(): React.JSX.Element {
    const { t } = useTranslation();
    const config = useCaptionConfig();
    const updateConfig = useSetConfig();

    const lockWindow = config.lockWindow ?? false;
    const alwaysOnTop = config.alwaysOnTop ?? true;
    const startOnLaunch = config.startOnLaunch ?? false;
    const captionWindowWidth = config.captionWindowWidth ?? 800;
    const captionFontSize = config.captionFontSize ?? 24;
    const captionFontColor = config.captionFontColor || '#ffffff';
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
            <SettingsSection
                title={t('settings.subtitle_behavior_title')}
                icon={<SlidersHorizontal size={20} />}
            >
                <SettingsItem
                    title={t('live.start_on_launch')}
                    hint={t('live.start_on_launch_hint')}
                >
                    <Switch
                        checked={startOnLaunch}
                        onChange={(enabled) => updateConfig({ startOnLaunch: enabled })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('live.always_on_top')}
                    hint={t('live.always_on_top_hint')}
                >
                    <Switch
                        checked={alwaysOnTop}
                        onChange={(enabled) => updateConfig({ alwaysOnTop: enabled })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('live.lock_window')}
                    hint={t('live.lock_window_hint')}
                >
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
                <SettingsItem
                    title={t('live.window_width')}
                >
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

                <SettingsItem
                    title={t('live.font_size')}
                >
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

                <SettingsItem
                    title={t('live.font_color')}
                >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                        <div
                            style={{
                                width: '36px',
                                height: '36px',
                                borderRadius: 'var(--radius-sm)',
                                border: '1px solid var(--color-border)',
                                overflow: 'hidden',
                                flexShrink: 0
                            }}
                        >
                            <input
                                type="color"
                                value={captionFontColor}
                                onChange={(e) => updateConfig({ captionFontColor: e.target.value })}
                                aria-label={t('live.font_color')}
                                style={{
                                    width: '150%',
                                    height: '150%',
                                    padding: 0,
                                    margin: '-25%',
                                    border: 'none',
                                    cursor: 'pointer',
                                    background: 'none'
                                }}
                            />
                        </div>
                        <input
                            type="text"
                            value={captionFontColor}
                            onChange={(e) => updateConfig({ captionFontColor: e.target.value })}
                            placeholder={t('live.font_color_hex_placeholder')}
                            maxLength={7}
                            aria-label={t('live.font_color_hex')}
                            className="settings-input"
                            style={{ width: '100px', fontFamily: 'monospace', textAlign: 'center' }}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('live.background_opacity')}
                >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '12px', width: '212px' }}>
                        <input
                            type="range"
                            min="0"
                            max="100"
                            step="1"
                            value={Math.round(captionBackgroundOpacity * 100)}
                            onChange={(e) => updateConfig({ captionBackgroundOpacity: Number(e.target.value) / 100 })}
                            className="settings-slider"
                            style={{ flex: 1 }}
                        />
                        <span style={{ width: '40px', textAlign: 'right', fontSize: '13px', fontVariantNumeric: 'tabular-nums' }}>
                            {Math.round(captionBackgroundOpacity * 100)}%
                        </span>
                    </div>
                </SettingsItem>
            </SettingsSection>

            <VoiceTypingSettingsSection />
        </SettingsTabContainer>
    );
}
