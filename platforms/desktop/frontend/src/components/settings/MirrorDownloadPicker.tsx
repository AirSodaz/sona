import type React from 'react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useModelConfig, useSetConfig } from '../../stores/configStore';
import { Dropdown, type DropdownOption } from '../Dropdown';

/**
 * Section-level download-mirror picker. Rendered in the section header rather
 * than the filter toolbar because it is a persistent download preference,
 * not a filter criterion.
 */
export function MirrorDownloadPicker(): React.JSX.Element {
  const { t } = useTranslation();
  const modelConfig = useModelConfig();
  const updateConfig = useSetConfig();

  const mirrorOptions: DropdownOption[] = useMemo(
    () => [
      {
        value: 'auto',
        label: t('settings.model_download_mirror_auto', { defaultValue: 'Auto' }),
      },
      {
        value: 'direct',
        label: t('settings.model_download_mirror_direct', { defaultValue: 'Direct (Official)' }),
      },
      {
        value: 'ghproxy',
        label: t('settings.model_download_mirror_ghproxy'),
        group: t('settings.model_download_mirror_group_github', { defaultValue: 'GitHub' }),
      },
      {
        value: 'ghnet',
        label: t('settings.model_download_mirror_ghnet'),
        group: t('settings.model_download_mirror_group_github', { defaultValue: 'GitHub' }),
      },
      {
        value: 'hf-mirror',
        label: t('settings.model_download_mirror_hfmirror', {
          defaultValue: 'Mirror (hf-mirror.com)',
        }),
        group: t('settings.model_download_mirror_group_hf', { defaultValue: 'Hugging Face' }),
      },
    ],
    [t]
  );

  return (
    <div className="settings-mirror-picker">
      <span className="settings-mirror-picker-label">{t('settings.model_download_mirror')}</span>
      <Dropdown
        id="settings-download-mirror"
        aria-label={t('settings.model_download_mirror')}
        value={modelConfig.modelDownloadMirror || 'auto'}
        onChange={(value) => updateConfig({ modelDownloadMirror: value })}
        options={mirrorOptions}
        style={{ width: '150px' }}
      />
    </div>
  );
}
