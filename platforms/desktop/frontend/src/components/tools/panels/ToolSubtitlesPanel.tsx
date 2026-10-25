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
import {
  isAsrRequestConfigured,
  resolveAsrTranscriptionRequest,
} from '../../../services/asrConfigService';
import { useCaptionConfig, useConfigStore, useSetConfig } from '../../../stores/configStore';
import { useTranscriptRuntimeStore } from '../../../stores/transcriptRuntimeStore';
import { hexToRgba } from '../../../utils/colorUtils';
import { ColorSwatchPicker } from '../../ColorSwatchPicker';
import { Dropdown } from '../../Dropdown';
import { Switch } from '../../Switch';
import { SettingsItem, SettingsSection, SettingsTabContainer } from '../../settings/SettingsLayout';
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
  const captionFontSize = config.captionFontSize ?? 24;
  const captionFontColor = config.captionFontColor || '#ffffff';
  const captionTranslationFontSize = config.captionTranslationFontSize ?? 18;
  const captionTranslationFontColor = config.captionTranslationFontColor || '#6ee7b7';
  const captionBackgroundColor = config.captionBackgroundColor || '#000000';
  const captionBackgroundOpacity = config.captionBackgroundOpacity ?? 0.6;
  const realtimeTranslationEnabled = config.realtimeTranslationEnabled ?? false;
  const realtimeTranslationSourceLanguage = config.realtimeTranslationSourceLanguage || 'en';
  const realtimeTranslationTargetLanguage = config.realtimeTranslationTargetLanguage || 'zh';
  const captionDisplayMode = config.captionDisplayMode || 'bilingual';
  const captionBilingualOrder = config.captionBilingualOrder || 'original_first';

  const translationLanguages = useMemo(
    () => [
      { value: 'zh', label: '中文 (Chinese)' },
      { value: 'en', label: 'English' },
      { value: 'ja', label: '日本語 (Japanese)' },
    ],
    []
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

  const originalLine = (
    <p
      key="original"
      className="subtitle-live-preview-line-primary"
      data-testid="subtitle-preview-original"
      style={{
        fontSize: `${captionFontSize}px`,
        color: captionFontColor,
        margin: 0,
        fontWeight: 500,
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
        fontSize: `${captionTranslationFontSize}px`,
        color: captionTranslationFontColor,
        margin: 0,
        opacity: 0.9,
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
              onChange={(enabled) => updateConfig({ realtimeTranslationEnabled: enabled })}
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
                    options={translationLanguages}
                    value={realtimeTranslationSourceLanguage}
                    onChange={(val) => updateConfig({ realtimeTranslationSourceLanguage: val })}
                    style={{ width: '150px' }}
                  />
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() =>
                      updateConfig({
                        realtimeTranslationSourceLanguage: realtimeTranslationTargetLanguage,
                        realtimeTranslationTargetLanguage: realtimeTranslationSourceLanguage,
                      })
                    }
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
                    options={translationLanguages}
                    value={realtimeTranslationTargetLanguage}
                    onChange={(val) => updateConfig({ realtimeTranslationTargetLanguage: val })}
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
                  onChange={(val) =>
                    updateConfig({
                      captionDisplayMode: val as 'bilingual' | 'translation' | 'original',
                    })
                  }
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
                    value={captionBilingualOrder}
                    onChange={(val) =>
                      updateConfig({
                        captionBilingualOrder: val as 'original_first' | 'translation_first',
                      })
                    }
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
                onChange={(e) =>
                  updateConfig({ captionTranslationFontSize: Number(e.target.value) })
                }
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
              }}
            >
              {(!realtimeTranslationEnabled || captionDisplayMode === 'original') && originalLine}
              {realtimeTranslationEnabled &&
                captionDisplayMode === 'translation' &&
                translationLine}
              {realtimeTranslationEnabled &&
                captionDisplayMode === 'bilingual' &&
                (captionBilingualOrder === 'translation_first' ? (
                  <>
                    {translationLine}
                    {originalLine}
                  </>
                ) : (
                  <>
                    {originalLine}
                    {translationLine}
                  </>
                ))}
            </div>
          </div>
        </SettingsSection>
      </div>
    </SettingsTabContainer>
  );
}
