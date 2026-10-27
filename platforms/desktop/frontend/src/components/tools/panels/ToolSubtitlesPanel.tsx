import {
  AlertTriangle,
  ArrowLeftRight,
  Eye,
  Languages,
  SlidersHorizontal,
  Subtitles,
} from 'lucide-react';
import type React from 'react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useRealtimeTranslationConfig } from '../../../hooks/useRealtimeTranslationConfig';
import {
  isAsrRequestConfigured,
  resolveAsrTranscriptionRequest,
} from '../../../services/asrConfigService';
import { captionWindowService } from '../../../services/captionWindowService';
import { useCaptionConfig, useConfigStore, useSetConfig } from '../../../stores/configStore';
import { useTranscriptRuntimeStore } from '../../../stores/transcriptRuntimeStore';
import { hexToRgba } from '../../../utils/colorUtils';
import { computeCaptionTextStyle } from '../../CaptionWindow';
import { ColorSwatchPicker } from '../../ColorSwatchPicker';
import { Dropdown } from '../../Dropdown';
import { Switch } from '../../Switch';
import {
  SegmentedSubTabs,
  SettingsItem,
  SettingsSection,
  SettingsTabContainer,
} from '../../settings/SettingsLayout';
import { SettingsShortcutInput } from '../../settings/SettingsShortcutInput';
import { ToolHeroHeader, type ToolHeroStatusTone } from '../ToolHeroHeader';

const SUBTITLE_COLOR_PRESETS = ['#ffffff', '#000000', '#facc15', '#60a5fa', '#4ade80', '#f87171'];

export function ToolSubtitlesPanel(): React.JSX.Element {
  const { t } = useTranslation();
  const config = useCaptionConfig();
  const updateConfig = useSetConfig();
  const isCaptionConfigured = useConfigStore((state) =>
    isAsrRequestConfigured(resolveAsrTranscriptionRequest(state.config, 'caption'))
  );
  const isCaptionMode = useTranscriptRuntimeStore((state) => state.isCaptionMode);
  const setIsCaptionMode = useTranscriptRuntimeStore((state) => state.setIsCaptionMode);

  const statusTone: ToolHeroStatusTone = !isCaptionMode
    ? 'inactive'
    : !isCaptionConfigured
      ? 'warning'
      : 'active';
  const statusText = !isCaptionMode
    ? t('tools.status_stopped', { defaultValue: 'Stopped' })
    : !isCaptionConfigured
      ? t('tools.status_needs_config', { defaultValue: 'Needs Config' })
      : t('tools.status_running', { defaultValue: 'Running' });

  const lockWindow = config.lockWindow ?? false;
  const alwaysOnTop = config.alwaysOnTop ?? true;
  const startOnLaunch = config.startOnLaunch ?? false;
  const captionWindowWidth = config.captionWindowWidth ?? 800;
  const captionFontSize = config.captionFontSize ?? 22;
  const captionFontColor = config.captionFontColor || '#ffffff';
  const captionTranslationFontSize = config.captionTranslationFontSize ?? 16;
  const captionTranslationFontColor = config.captionTranslationFontColor || '#6ee7b7';
  const captionBackgroundColor = config.captionBackgroundColor || '#17171a';
  const captionBackgroundOpacity = config.captionBackgroundOpacity ?? 0.72;
  const {
    realtimeTranslationEnabled,
    realtimeTranslationSourceLanguage,
    captionDisplayMode,
    sourceLanguages,
    targetLanguages,
    targetLanguageValue,
    handleSourceLanguageChange,
    handleTargetLanguageChange,
    handleSwapLanguages,
    handleToggleRealtimeTranslation,
  } = useRealtimeTranslationConfig();
  const captionBilingualOrder = config.captionBilingualOrder || 'original_first';

  const captionTextStroke = config.captionTextStroke ?? false;
  const captionStrokeWidth = config.captionStrokeWidth ?? 2;
  const captionStrokeColor = config.captionStrokeColor || '#000000';
  const captionTextShadow = config.captionTextShadow ?? true;
  const captionTextAlign = config.captionTextAlign ?? 'center';
  const captionDisappearDelay = config.captionDisappearDelay ?? 4.0;
  const captionLockShortcut = config.captionLockShortcut || 'Alt+C';

  const captionDisplayPriority:
    | 'translation_main'
    | 'original_main'
    | 'translation_only'
    | 'original_only' =
    config.captionDisplayPriority ??
    (captionDisplayMode === 'translation'
      ? 'translation_only'
      : captionDisplayMode === 'original'
        ? 'original_only'
        : captionBilingualOrder === 'translation_first'
          ? 'translation_main'
          : 'original_main');
  const effectiveBilingualOrder: 'original_first' | 'translation_first' =
    captionDisplayPriority === 'translation_main'
      ? 'translation_first'
      : captionDisplayPriority === 'original_main'
        ? 'original_first'
        : config.captionBilingualOrder || 'original_first';

  const isTranslationMain = captionDisplayPriority === 'translation_main';
  const isOriginalMain = captionDisplayPriority === 'original_main';
  const isTranslationOnly = captionDisplayPriority === 'translation_only';
  const isOriginalOnly = captionDisplayPriority === 'original_only';
  const captionStyle = useMemo(
    () => ({
      width: captionWindowWidth,
      fontSize: captionFontSize,
      color: captionFontColor,
      backgroundColor: captionBackgroundColor,
      backgroundOpacity: captionBackgroundOpacity,
      textStroke: captionTextStroke,
      strokeWidth: captionStrokeWidth,
      strokeColor: captionStrokeColor,
      textShadow: captionTextShadow,
      textAlign: captionTextAlign,
      disappearDelay: captionDisappearDelay,
      displayPriority: captionDisplayPriority,
    }),
    [
      captionWindowWidth,
      captionFontSize,
      captionFontColor,
      captionBackgroundColor,
      captionBackgroundOpacity,
      captionTextStroke,
      captionStrokeWidth,
      captionStrokeColor,
      captionTextShadow,
      captionTextAlign,
      captionDisappearDelay,
      captionDisplayPriority,
    ]
  );

  const displayModeOptions = useMemo(
    () => [
      {
        value: 'bilingual',
        label: t('settings.caption_mode_bilingual', {
          defaultValue: 'Bilingual (Original & Translation)',
        }),
      },
      {
        value: 'translation',
        label: t('settings.caption_mode_translation', { defaultValue: 'Translation Only' }),
      },
      {
        value: 'original',
        label: t('settings.caption_mode_original', { defaultValue: 'Original Only' }),
      },
    ],
    [t]
  );

  const bilingualOrderOptions = useMemo(
    () => [
      {
        value: 'original_first',
        label: t('settings.caption_order_original_first', { defaultValue: 'Original on Top' }),
      },
      {
        value: 'translation_first',
        label: t('settings.caption_order_translation_first', {
          defaultValue: 'Translation on Top',
        }),
      },
    ],
    [t]
  );

  const hasTranslation = realtimeTranslationEnabled && !isOriginalOnly;

  const originalLine = (
    <p
      key="original"
      className="subtitle-live-preview-line-primary"
      data-testid="subtitle-preview-original"
      style={{
        fontSize:
          isTranslationMain && hasTranslation
            ? `${captionTranslationFontSize}px`
            : `${captionFontSize}px`,
        color: captionFontColor,
        margin: 0,
        opacity: isTranslationMain && hasTranslation ? 0.8 : 1,
        fontWeight: isTranslationMain && hasTranslation ? 400 : 500,
        textAlign: captionTextAlign,
        ...computeCaptionTextStyle(captionStyle, !(isTranslationMain && hasTranslation), false),
      }}
    >
      {t('tools.subtitles_preview_text_primary', {
        defaultValue: 'Sona Live Speech Recognition and Subtitles',
      })}
    </p>
  );

  const translationLine = (
    <p
      key="translation"
      className="subtitle-live-preview-line-secondary"
      data-testid="subtitle-preview-translation"
      style={{
        fontSize: isOriginalMain ? `${captionTranslationFontSize}px` : `${captionFontSize}px`,
        color: captionTranslationFontColor,
        margin: 0,
        opacity: 1,
        fontWeight: isOriginalMain ? 400 : 500,
        textAlign: captionTextAlign,
        ...computeCaptionTextStyle(captionStyle, !isOriginalMain, true),
      }}
    >
      {t('tools.subtitles_preview_text_secondary', {
        defaultValue: 'Real-time speech recognition and bilingual live subtitle preview.',
      })}
    </p>
  );

  const handleToggleCaptionMode = (next?: boolean) => {
    setIsCaptionMode(typeof next === 'boolean' ? next : !isCaptionMode);
  };

  const previewBgColor = hexToRgba(captionBackgroundColor, captionBackgroundOpacity);

  return (
    <SettingsTabContainer id="tools-panel-subtitles" ariaLabelledby="tools-tab-subtitles">
      <ToolHeroHeader
        icon={<Subtitles size={24} />}
        title={t('tools.subtitles', { defaultValue: 'Live Subtitles' })}
        description={t('settings.subtitle_tab_description', {
          defaultValue: 'Configure live caption window behavior and visual appearance.',
        })}
        statusText={statusText}
        statusTone={statusTone}
        enabled={isCaptionMode}
        onToggle={handleToggleCaptionMode}
        switchAriaLabel={t('live.caption_mode', { defaultValue: 'Live Caption' })}
      />

      <div className="tools-panel-body">
        {!isCaptionConfigured && isCaptionMode && (
          <div
            className="settings-warning-notice"
            role="alert"
            style={{ marginBottom: '16px', display: 'flex', alignItems: 'center', gap: '8px' }}
          >
            <AlertTriangle size={16} style={{ flexShrink: 0 }} />
            <span>
              {t('tools.subtitles_missing_model_warning', {
                defaultValue:
                  'Live Subtitles requires a streaming ASR model. Please configure a streaming model in Model Settings.',
              })}
            </span>
          </div>
        )}
        <SettingsSection
          id="tools-subtitle-window"
          title={t('settings.subtitle_behavior_title', { defaultValue: 'Window Behavior' })}
          icon={<SlidersHorizontal size={20} />}
        >
          <SettingsItem
            title={t('live.start_on_launch', { defaultValue: 'Start on Launch' })}
            hint={t('live.start_on_launch_hint', {
              defaultValue: 'Automatically start Live Caption when the program opens',
            })}
          >
            <Switch
              checked={startOnLaunch}
              onChange={(enabled) => updateConfig({ startOnLaunch: enabled })}
              aria-label={t('live.start_on_launch', { defaultValue: 'Start on Launch' })}
            />
          </SettingsItem>

          <SettingsItem
            title={t('live.always_on_top', { defaultValue: 'Always on Top' })}
            hint={t('live.always_on_top_hint', {
              defaultValue: 'Keep window above others',
            })}
          >
            <Switch
              checked={alwaysOnTop}
              onChange={(enabled) => updateConfig({ alwaysOnTop: enabled })}
              aria-label={t('live.always_on_top', { defaultValue: 'Always on Top' })}
            />
          </SettingsItem>

          <SettingsItem
            title={t('live.lock_window', { defaultValue: 'Lock Window' })}
            hint={t('live.lock_window_hint', {
              defaultValue: 'Make window click-through',
            })}
          >
            <Switch
              checked={lockWindow}
              onChange={(enabled) => updateConfig({ lockWindow: enabled })}
              aria-label={t('live.lock_window', { defaultValue: 'Lock Window' })}
            />
          </SettingsItem>

          <SettingsItem
            title={t('settings.caption_lock_shortcut', {
              defaultValue: 'Lock Click-Through Shortcut',
            })}
            hint={t('settings.caption_lock_shortcut_hint', {
              defaultValue:
                'Global hotkey to toggle mouse click-through for the live caption window.',
            })}
          >
            <SettingsShortcutInput
              value={captionLockShortcut}
              onChange={(val) => updateConfig({ captionLockShortcut: val })}
            />
          </SettingsItem>
          <SettingsItem
            title={t('settings.caption_reset_position', {
              defaultValue: 'Reset Window Position',
            })}
            hint={t('settings.caption_reset_position_hint', {
              defaultValue:
                'Reset the caption floating window to the default bottom-center position.',
            })}
          >
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => void captionWindowService.resetPosition()}
            >
              {t('settings.caption_reset_position', {
                defaultValue: 'Reset Window Position',
              })}
            </button>
          </SettingsItem>
        </SettingsSection>
        <SettingsSection
          id="tools-subtitle-translation"
          title={t('settings.subtitle_realtime_translation_title', {
            defaultValue: 'Real-time Bilingual Translation',
          })}
          icon={<Languages size={20} />}
          description={t('settings.subtitle_realtime_translation_desc', {
            defaultValue:
              'Configure machine translation, display modes, and bilingual ordering for live subtitles.',
          })}
        >
          <SettingsItem
            title={t('settings.enable_realtime_translation', {
              defaultValue: 'Enable Real-time Translation',
            })}
            hint={t('settings.enable_realtime_translation_hint', {
              defaultValue: 'Synchronously translate and display subtitles in real time',
            })}
          >
            <Switch
              checked={realtimeTranslationEnabled}
              onChange={handleToggleRealtimeTranslation}
              aria-label={t('settings.enable_realtime_translation', {
                defaultValue: 'Enable Real-time Translation',
              })}
            />
          </SettingsItem>

          {realtimeTranslationEnabled && (
            <>
              <SettingsItem
                title={t('settings.translation_language_pair', {
                  defaultValue: 'Language Pair',
                })}
                hint={t('settings.translation_language_pair_hint', {
                  defaultValue: 'Select source and target translation languages',
                })}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <Dropdown
                    id="tools-subtitle-source-lang"
                    aria-label={t('settings.translation_source_language', {
                      defaultValue: 'Source Language',
                    })}
                    options={sourceLanguages}
                    value={realtimeTranslationSourceLanguage}
                    onChange={handleSourceLanguageChange}
                    style={{ width: '150px' }}
                  />
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={handleSwapLanguages}
                    title={t('settings.swap_translation_languages', {
                      defaultValue: 'Swap languages',
                    })}
                    aria-label={t('settings.swap_translation_languages', {
                      defaultValue: 'Swap languages',
                    })}
                    style={{
                      padding: '6px 10px',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                    }}
                  >
                    <ArrowLeftRight size={14} />
                  </button>
                  <Dropdown
                    id="tools-subtitle-target-lang"
                    aria-label={t('settings.translation_target_language', {
                      defaultValue: 'Target Language',
                    })}
                    options={targetLanguages}
                    value={targetLanguageValue}
                    onChange={handleTargetLanguageChange}
                    style={{ width: '150px' }}
                  />
                </div>
              </SettingsItem>

              <SettingsItem
                title={t('settings.caption_display_mode', {
                  defaultValue: 'Subtitle Display Mode',
                })}
              >
                <Dropdown
                  id="tools-subtitle-display-mode"
                  aria-label={t('settings.caption_display_mode', {
                    defaultValue: 'Subtitle Display Mode',
                  })}
                  options={displayModeOptions}
                  value={captionDisplayMode}
                  onChange={(val) => {
                    const mode = val as 'bilingual' | 'translation' | 'original';
                    const priority =
                      mode === 'translation'
                        ? 'translation_only'
                        : mode === 'original'
                          ? 'original_only'
                          : captionBilingualOrder === 'translation_first'
                            ? 'translation_main'
                            : 'original_main';
                    updateConfig({
                      captionDisplayMode: mode,
                      captionDisplayPriority: priority,
                    });
                  }}
                  style={{ width: '260px' }}
                />
              </SettingsItem>

              {captionDisplayMode === 'bilingual' && (
                <SettingsItem
                  title={t('settings.caption_bilingual_order', {
                    defaultValue: 'Bilingual Order',
                  })}
                >
                  <Dropdown
                    id="tools-subtitle-bilingual-order"
                    aria-label={t('settings.caption_bilingual_order', {
                      defaultValue: 'Bilingual Order',
                    })}
                    options={bilingualOrderOptions}
                    value={effectiveBilingualOrder}
                    onChange={(val) => {
                      const order = val as 'original_first' | 'translation_first';
                      updateConfig({
                        captionBilingualOrder: order,
                        captionDisplayPriority:
                          order === 'translation_first' ? 'translation_main' : 'original_main',
                      });
                    }}
                    style={{ width: '200px' }}
                  />
                </SettingsItem>
              )}
            </>
          )}
        </SettingsSection>

        <SettingsSection
          id="tools-subtitle-appearance"
          title={t('settings.subtitle_appearance_title', { defaultValue: 'Appearance' })}
          icon={<Subtitles size={20} />}
          description={t('settings.subtitle_appearance_desc', {
            defaultValue: 'Customize the look of your live captions.',
          })}
        >
          <SettingsItem title={t('live.window_width', { defaultValue: 'Floating Window Width' })}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <input
                type="range"
                min="400"
                max="1600"
                step="10"
                value={captionWindowWidth}
                onChange={(e) => updateConfig({ captionWindowWidth: Number(e.target.value) })}
                aria-label={t('live.window_width', { defaultValue: 'Floating Window Width' })}
                className="settings-slider"
                style={{ width: '160px' }}
              />
              <span
                style={{ minWidth: '55px', fontSize: '13px', fontVariantNumeric: 'tabular-nums' }}
              >
                {captionWindowWidth}px
              </span>
            </div>
          </SettingsItem>

          <SettingsItem
            title={t('live.original_font_size', {
              defaultValue: t('live.font_size', { defaultValue: 'Original Font Size' }),
            })}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <input
                type="range"
                min="14"
                max="48"
                step="1"
                value={captionFontSize}
                onChange={(e) => updateConfig({ captionFontSize: Number(e.target.value) })}
                aria-label={t('live.original_font_size', { defaultValue: 'Original Font Size' })}
                className="settings-slider"
                style={{ width: '160px' }}
              />
              <span
                style={{ minWidth: '45px', fontSize: '13px', fontVariantNumeric: 'tabular-nums' }}
              >
                {captionFontSize}px
              </span>
            </div>
          </SettingsItem>

          <SettingsItem
            title={t('live.original_font_color', {
              defaultValue: t('live.font_color', { defaultValue: 'Original Font Color' }),
            })}
          >
            <ColorSwatchPicker
              value={captionFontColor}
              onChange={(color) => updateConfig({ captionFontColor: color })}
              presets={SUBTITLE_COLOR_PRESETS}
              aria-label={t('live.original_font_color', { defaultValue: 'Original Font Color' })}
            />
          </SettingsItem>

          <SettingsItem
            title={t('live.translation_font_size', {
              defaultValue: 'Translation Font Size',
            })}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <input
                type="range"
                min="12"
                max="48"
                step="1"
                value={captionTranslationFontSize}
                onChange={(e) => {
                  const parsed = Number(e.target.value);
                  if (!Number.isNaN(parsed)) {
                    updateConfig({
                      captionTranslationFontSize: Math.max(12, Math.min(48, parsed)),
                    });
                  }
                }}
                aria-label={t('live.translation_font_size', {
                  defaultValue: 'Translation Font Size',
                })}
                className="settings-slider"
                style={{ width: '160px' }}
              />
              <span
                style={{ minWidth: '45px', fontSize: '13px', fontVariantNumeric: 'tabular-nums' }}
              >
                {captionTranslationFontSize}px
              </span>
            </div>
          </SettingsItem>

          <SettingsItem
            title={t('live.translation_font_color', {
              defaultValue: 'Translation Font Color',
            })}
          >
            <ColorSwatchPicker
              value={captionTranslationFontColor}
              onChange={(color) => updateConfig({ captionTranslationFontColor: color })}
              presets={SUBTITLE_COLOR_PRESETS}
              aria-label={t('live.translation_font_color', {
                defaultValue: 'Translation Font Color',
              })}
            />
          </SettingsItem>

          <SettingsItem title={t('live.background_color', { defaultValue: 'Background Color' })}>
            <ColorSwatchPicker
              value={captionBackgroundColor}
              onChange={(color) => updateConfig({ captionBackgroundColor: color })}
              presets={SUBTITLE_COLOR_PRESETS}
              aria-label={t('live.background_color', { defaultValue: 'Background Color' })}
            />
          </SettingsItem>

          <SettingsItem
            title={t('live.background_opacity', { defaultValue: 'Background Opacity' })}
          >
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
                aria-label={t('live.background_opacity', { defaultValue: 'Background Opacity' })}
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

          <SettingsItem
            title={t('settings.caption_text_align', { defaultValue: 'Text Alignment' })}
          >
            <SegmentedSubTabs<'center' | 'left'>
              id="tools-caption-text-align-select"
              role="radiogroup"
              size="sm"
              ariaLabel={t('settings.caption_text_align', { defaultValue: 'Text Alignment' })}
              items={[
                {
                  value: 'center',
                  label: t('settings.caption_align_center', { defaultValue: 'Center' }),
                },
                {
                  value: 'left',
                  label: t('settings.caption_align_left', { defaultValue: 'Left' }),
                },
              ]}
              activeTab={captionTextAlign}
              onChange={(val) => updateConfig({ captionTextAlign: val })}
            />
          </SettingsItem>

          <SettingsItem
            title={t('settings.caption_text_stroke', { defaultValue: 'Text Stroke' })}
            hint={t('settings.caption_text_stroke_hint', {
              defaultValue: 'Adds a high-contrast outline for readability on bright backgrounds.',
            })}
          >
            <Switch
              checked={captionTextStroke}
              onChange={(enabled) =>
                updateConfig({
                  captionTextStroke: enabled,
                  ...(enabled && captionStrokeWidth <= 0 ? { captionStrokeWidth: 2 } : {}),
                })
              }
              aria-label={t('settings.caption_text_stroke', { defaultValue: 'Text Stroke' })}
            />
          </SettingsItem>

          {captionTextStroke && (
            <>
              <SettingsItem
                title={t('settings.caption_stroke_width', { defaultValue: 'Stroke Width' })}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px', width: '212px' }}>
                  <input
                    type="range"
                    min="1"
                    max="5"
                    step="0.5"
                    value={captionStrokeWidth}
                    onChange={(e) => updateConfig({ captionStrokeWidth: Number(e.target.value) })}
                    className="settings-slider"
                    style={{ flex: 1 }}
                    aria-label={t('settings.caption_stroke_width', {
                      defaultValue: 'Stroke Width',
                    })}
                  />
                  <span
                    style={{
                      width: '40px',
                      textAlign: 'right',
                      fontSize: '13px',
                      fontVariantNumeric: 'tabular-nums',
                    }}
                  >
                    {captionStrokeWidth}px
                  </span>
                </div>
              </SettingsItem>

              <SettingsItem
                title={t('settings.caption_stroke_color', { defaultValue: 'Stroke Color' })}
              >
                <ColorSwatchPicker
                  value={captionStrokeColor}
                  onChange={(color) => updateConfig({ captionStrokeColor: color })}
                  presets={SUBTITLE_COLOR_PRESETS}
                  aria-label={t('settings.caption_stroke_color', { defaultValue: 'Stroke Color' })}
                />
              </SettingsItem>
            </>
          )}

          <SettingsItem
            title={t('settings.caption_text_shadow', { defaultValue: 'Text Shadow' })}
            hint={t('settings.caption_text_shadow_hint', {
              defaultValue: 'Soft drop shadow behind caption text.',
            })}
          >
            <Switch
              checked={captionTextShadow}
              onChange={(enabled) => updateConfig({ captionTextShadow: enabled })}
              aria-label={t('settings.caption_text_shadow', { defaultValue: 'Text Shadow' })}
            />
          </SettingsItem>

          <SettingsItem
            title={t('settings.caption_disappear_delay', { defaultValue: 'Disappear Delay' })}
            hint={t('settings.caption_disappear_delay_hint', {
              defaultValue: 'Time in seconds before subtitles fade out after speech stops.',
            })}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px', width: '212px' }}>
              <input
                type="range"
                min="1"
                max="10"
                step="0.5"
                value={captionDisappearDelay}
                onChange={(e) => updateConfig({ captionDisappearDelay: Number(e.target.value) })}
                className="settings-slider"
                style={{ flex: 1 }}
                aria-label={t('settings.caption_disappear_delay', {
                  defaultValue: 'Disappear Delay',
                })}
              />
              <span
                style={{
                  width: '40px',
                  textAlign: 'right',
                  fontSize: '13px',
                  fontVariantNumeric: 'tabular-nums',
                }}
              >
                {captionDisappearDelay.toFixed(1)}s
              </span>
            </div>
          </SettingsItem>
        </SettingsSection>

        {/* Real-time WYSIWYG Subtitle Preview */}
        <SettingsSection
          id="tools-subtitle-preview-section"
          title={t('tools.subtitles_preview_title', { defaultValue: 'Live Subtitle Preview' })}
          icon={<Eye size={20} />}
          description={t('tools.subtitles_preview_desc', {
            defaultValue: 'WYSIWYG simulation of the floating subtitle appearance on your desktop.',
          })}
        >
          <div className="subtitle-live-preview-box">
            <div
              className="subtitle-live-preview-stage"
              data-testid="subtitle-preview-stage"
              style={{
                width: '100%',
                maxWidth: `${Math.min(captionWindowWidth, 720)}px`,
                backgroundColor: previewBgColor,
                display: 'flex',
                flexDirection: 'column',
                gap: '6px',
                textAlign: captionTextAlign,
              }}
            >
              {(!realtimeTranslationEnabled || isOriginalOnly) && originalLine}
              {realtimeTranslationEnabled && isTranslationOnly && translationLine}
              {realtimeTranslationEnabled && isTranslationMain && (
                <>
                  {translationLine}
                  {originalLine}
                </>
              )}
              {realtimeTranslationEnabled && isOriginalMain && (
                <>
                  {originalLine}
                  {translationLine}
                </>
              )}
            </div>
          </div>
        </SettingsSection>
      </div>
    </SettingsTabContainer>
  );
}
