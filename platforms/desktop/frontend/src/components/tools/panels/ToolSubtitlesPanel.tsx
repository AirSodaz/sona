import { Eye, SlidersHorizontal, Subtitles } from 'lucide-react';
import type React from 'react';
import { useTranslation } from 'react-i18next';
import { useCaptionConfig, useSetConfig } from '../../../stores/configStore';
import { useTranscriptRuntimeStore } from '../../../stores/transcriptRuntimeStore';
import { hexToRgba } from '../../../utils/colorUtils';
import { ColorSwatchPicker } from '../../ColorSwatchPicker';
import { Switch } from '../../Switch';
import { SettingsItem, SettingsSection, SettingsTabContainer } from '../../settings/SettingsLayout';
import { ToolHeroHeader } from '../ToolHeroHeader';

export function ToolSubtitlesPanel(): React.JSX.Element {
  const { t } = useTranslation();
  const config = useCaptionConfig();
  const updateConfig = useSetConfig();

  const isCaptionMode = useTranscriptRuntimeStore((state) => state.isCaptionMode);
  const setIsCaptionMode = useTranscriptRuntimeStore((state) => state.setIsCaptionMode);

  const lockWindow = config.lockWindow ?? false;
  const alwaysOnTop = config.alwaysOnTop ?? true;
  const startOnLaunch = config.startOnLaunch ?? false;
  const captionWindowWidth = config.captionWindowWidth ?? 800;
  const captionFontSize = config.captionFontSize ?? 24;
  const captionFontColor = config.captionFontColor || '#ffffff';
  const captionBackgroundColor = config.captionBackgroundColor || '#000000';
  const captionBackgroundOpacity = config.captionBackgroundOpacity ?? 0.6;

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
        statusText={
          isCaptionMode
            ? t('tools.status_running', { defaultValue: 'Running' })
            : t('tools.status_stopped', { defaultValue: 'Stopped' })
        }
        statusTone={isCaptionMode ? 'active' : 'inactive'}
        enabled={isCaptionMode}
        onToggle={handleToggleCaptionMode}
        switchAriaLabel={t('live.caption_mode', { defaultValue: 'Live Caption' })}
      />

      <div className="tools-panel-body">
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

          <SettingsItem title={t('live.font_size', { defaultValue: 'Font Size' })}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <input
                type="range"
                min="14"
                max="48"
                step="1"
                value={captionFontSize}
                onChange={(e) => updateConfig({ captionFontSize: Number(e.target.value) })}
                aria-label={t('live.font_size', { defaultValue: 'Font Size' })}
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

          <SettingsItem title={t('live.font_color', { defaultValue: 'Font Color' })}>
            <ColorSwatchPicker
              value={captionFontColor}
              onChange={(color) => updateConfig({ captionFontColor: color })}
              aria-label={t('live.font_color', { defaultValue: 'Font Color' })}
            />
          </SettingsItem>

          <SettingsItem title={t('live.background_color', { defaultValue: 'Background Color' })}>
            <ColorSwatchPicker
              value={captionBackgroundColor}
              onChange={(color) => updateConfig({ captionBackgroundColor: color })}
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
              style={{
                width: '100%',
                maxWidth: `${Math.min(captionWindowWidth, 720)}px`,
                backgroundColor: previewBgColor,
                color: captionFontColor,
              }}
            >
              <p
                className="subtitle-live-preview-line-primary"
                style={{ fontSize: `${captionFontSize}px`, margin: 0, fontWeight: 500 }}
              >
                {t('tools.subtitles_preview_text_primary', {
                  defaultValue: 'Sona Live Speech Recognition and Subtitles',
                })}
              </p>
              <p
                className="subtitle-live-preview-line-secondary"
                style={{
                  fontSize: `${Math.max(12, Math.round(captionFontSize * 0.75))}px`,
                  margin: '4px 0 0 0',
                  opacity: 0.85,
                }}
              >
                {t('tools.subtitles_preview_text_secondary', {
                  defaultValue: 'Real-time speech recognition and bilingual live subtitle preview.',
                })}
              </p>
            </div>
          </div>
        </SettingsSection>
      </div>
    </SettingsTabContainer>
  );
}
