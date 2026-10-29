import { ExternalLink } from 'lucide-react';
import type React from 'react';
import { useTranslation } from 'react-i18next';
import { openUrl } from '../../../../services/tauri/platform/opener';
import { Modal } from '../../../Modal';
export interface EmbyPluginGuideModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function EmbyPluginGuideModal({
  isOpen,
  onClose,
}: EmbyPluginGuideModalProps): React.JSX.Element {
  const { t } = useTranslation();

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('emby.plugin_modal_title', {
        defaultValue: 'Install Emby.SubtitleManager Plugin',
      })}
      size="md"
      footer={
        <div className="emby-modal-footer-actions">
          <button type="button" className="btn btn-primary" onClick={onClose}>
            {t('common.close', { defaultValue: 'Close' })}
          </button>
        </div>
      }
    >
      <div className="emby-plugin-guide-body">
        <p className="emby-guide-intro">
          {t('emby.plugin_intro', {
            defaultValue:
              'Sona uses the open-source Emby.SubtitleManager plugin for seamless bidirectional subtitle uploads and metadata refresh. Follow these steps to deploy:',
          })}
        </p>

        <div className="emby-guide-step-card">
          <div className="emby-guide-step-title">
            {t('emby.plugin_step1_title', {
              defaultValue: 'Step 1: Download the plugin DLL',
            })}
          </div>
          <p className="emby-guide-step-desc">
            {t('emby.plugin_step1_desc', {
              defaultValue:
                'Get the latest Emby.SubtitleManager.dll from the official GitHub Release:',
            })}
          </p>
          <button
            type="button"
            onClick={async () => {
              try {
                await openUrl('https://github.com/dianbaer/Emby.SubtitleManager/releases');
              } catch {
                window.open(
                  'https://github.com/dianbaer/Emby.SubtitleManager/releases',
                  '_blank',
                  'noopener,noreferrer'
                );
              }
            }}
            className="btn btn-sm btn-secondary emby-github-link"
          >
            <ExternalLink size={14} />
            <span>{t('emby.plugin_step1_btn', { defaultValue: 'Visit GitHub Releases' })}</span>
          </button>
        </div>

        <div className="emby-guide-step-card">
          <div className="emby-guide-step-title">
            {t('emby.plugin_step2_title', {
              defaultValue: 'Step 2: Place the DLL in Emby plugins folder and restart',
            })}
          </div>
          <ul className="emby-guide-paths-list">
            <li>
              {t('emby.plugin_docker', {
                defaultValue: 'Docker deployment: mount volume path /config/plugins/',
              })}
            </li>
            <li>
              {t('emby.plugin_synology', {
                defaultValue: 'Synology NAS: /var/packages/EmbyServer/var/plugins/',
              })}
            </li>
            <li>
              {t('emby.plugin_windows', {
                defaultValue: 'Windows Server: %APPDATA%\\Emby-Server\\programdata\\plugins\\',
              })}
            </li>
            <li>
              {t('emby.plugin_linux', {
                defaultValue: 'Linux Native: /var/lib/emby/plugins/',
              })}
            </li>
          </ul>
        </div>

        <p className="emby-guide-tip">
          {t('emby.plugin_tip', {
            defaultValue:
              'Tip: After copying the DLL and restarting Emby, return to Sona and reconnect to activate full sync capabilities.',
          })}
        </p>
      </div>
    </Modal>
  );
}
