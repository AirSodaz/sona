import { ArrowRight, Bot, Keyboard, Server, Subtitles, Wrench } from 'lucide-react';
import type React from 'react';
import { useTranslation } from 'react-i18next';
import { useToolsStore } from '../../stores/toolsStore';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import type { ToolId } from '../../types/tools';
import { SettingsPageHeader, SettingsTabContainer } from './SettingsLayout';

export interface SettingsMigratedBannerProps {
  tool: ToolId;
  onClose?: () => void;
}
const TOOL_META_MAP: Record<
  ToolId,
  { titleKey: string; defaultTitle: string; icon: React.ReactElement }
> = {
  subtitles: {
    titleKey: 'tools.subtitles',
    defaultTitle: 'Live Subtitles',
    icon: <Subtitles width={28} height={28} />,
  },
  voice_typing: {
    titleKey: 'tools.voice_typing',
    defaultTitle: 'Voice Typing',
    icon: <Keyboard width={28} height={28} />,
  },
  api_server: {
    titleKey: 'tools.api_server',
    defaultTitle: 'API Server',
    icon: <Server width={28} height={28} />,
  },
  mcp: {
    titleKey: 'tools.mcp',
    defaultTitle: 'MCP Agents',
    icon: <Bot width={28} height={28} />,
  },
};

export function SettingsMigratedBanner({
  tool,
  onClose,
}: SettingsMigratedBannerProps): React.JSX.Element {
  const { t } = useTranslation();

  const meta = TOOL_META_MAP[tool] || TOOL_META_MAP.subtitles;
  const toolName = t(meta.titleKey, { defaultValue: meta.defaultTitle });

  const handleGoToTools = () => {
    useToolsStore.getState().setActiveTool(tool);
    useTranscriptRuntimeStore.getState().setMode('tools');
    onClose?.();
  };

  return (
    <SettingsTabContainer
      id={`settings-panel-migrated-${tool}`}
      ariaLabelledby={`settings-migrated-heading-${tool}`}
    >
      <SettingsPageHeader
        id={`settings-migrated-heading-${tool}`}
        icon={meta.icon}
        title={toolName}
        description={t('tools.migrated_notice_desc', {
          defaultValue:
            'Standalone extension and service tools have been consolidated into the unified homepage workbench.',
        })}
      />

      <div
        className="settings-migrated-card"
        style={{
          marginTop: '16px',
          padding: '28px 32px',
          background: 'var(--color-bg-secondary)',
          border: '1px solid var(--color-border)',
          borderRadius: 'var(--radius-lg, 12px)',
          display: 'flex',
          flexDirection: 'column',
          gap: '20px',
          boxShadow: 'var(--shadow-sm)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: '16px' }}>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              width: '44px',
              height: '44px',
              borderRadius: 'var(--radius-md, 8px)',
              background: 'var(--color-primary-subtle, rgba(59, 130, 246, 0.12))',
              color: 'var(--color-primary)',
              flexShrink: 0,
            }}
          >
            <Wrench size={22} aria-hidden="true" />
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
              <h3
                style={{
                  margin: 0,
                  fontSize: '1.05rem',
                  fontWeight: 600,
                  color: 'var(--color-text-primary)',
                }}
              >
                {toolName}
              </h3>
              <span
                style={{
                  fontSize: '0.75rem',
                  fontWeight: 500,
                  padding: '2px 8px',
                  borderRadius: '9999px',
                  background: 'var(--color-primary-subtle, rgba(59, 130, 246, 0.12))',
                  color: 'var(--color-primary)',
                }}
              >
                {t('tools.migrated_badge', { defaultValue: 'Moved to Tools' })}
              </span>
            </div>

            <p
              style={{
                margin: 0,
                fontSize: '0.875rem',
                color: 'var(--color-text-secondary)',
                lineHeight: 1.5,
              }}
            >
              {t('tools.migrated_notice', {
                defaultValue:
                  'This setting has been moved to the homepage Tools workbench for faster control, configuration, and monitoring.',
              })}
            </p>
          </div>
        </div>

        <div style={{ display: 'flex', justifyContent: 'flex-end', paddingTop: '8px' }}>
          <button
            type="button"
            className="btn btn-primary"
            onClick={handleGoToTools}
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: '8px',
              padding: '8px 20px',
            }}
          >
            <Wrench size={16} aria-hidden="true" />
            <span>{t('tools.go_to_tools', { defaultValue: 'Go to Tools' })}</span>
            <ArrowRight size={16} aria-hidden="true" />
          </button>
        </div>
      </div>
    </SettingsTabContainer>
  );
}

export default SettingsMigratedBanner;
