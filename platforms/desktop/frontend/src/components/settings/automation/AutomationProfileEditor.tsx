import React from 'react';
import { useTranslation } from 'react-i18next';
import { Checkbox } from '../../Checkbox';
import { Dropdown } from '../../Dropdown';
import { SettingsItem } from '../SettingsLayout';
import type { AutomationProfile } from '../../../types/automation';

export type AutomationProfileDraft = Omit<AutomationProfile, 'createdAt' | 'updatedAt'>;

type SelectOption = { value: string; label: string };
type NamedOption = { id: string; name: string };

type Props = {
    draft: AutomationProfileDraft;
    hotwordSets: NamedOption[];
    languageOptions: SelectOption[];
    onCancel: () => void;
    onChange: (draft: AutomationProfileDraft) => void;
    onSave: () => void;
    polishKeywordSets: NamedOption[];
    polishPresetOptions: SelectOption[];
    speakerProfiles: NamedOption[];
    summaryTemplateOptions: SelectOption[];
    textReplacementSets: NamedOption[];
};

function SetSelector({
    title,
    items,
    selectedIds,
    onChange,
}: {
    title: string;
    items: NamedOption[];
    selectedIds: string[];
    onChange: (ids: string[]) => void;
}): React.JSX.Element {
    const { t } = useTranslation();

    return (
        <SettingsItem title={title} layout="vertical">
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))', gap: '8px', width: '100%' }}>
                {items.length === 0 ? (
                    <span className="settings-item-hint">
                        {t('automation.none_configured', { defaultValue: 'None configured' })}
                    </span>
                ) : items.map((item) => (
                    <Checkbox
                        key={item.id}
                        checked={selectedIds.includes(item.id)}
                        label={item.name}
                        onChange={(checked) => onChange(
                            checked
                                ? Array.from(new Set([...selectedIds, item.id]))
                                : selectedIds.filter((id) => id !== item.id),
                        )}
                    />
                ))}
            </div>
        </SettingsItem>
    );
}

export function AutomationProfileEditor({
    draft,
    hotwordSets,
    languageOptions,
    onCancel,
    onChange,
    onSave,
    polishKeywordSets,
    polishPresetOptions,
    speakerProfiles,
    summaryTemplateOptions,
    textReplacementSets,
}: Props): React.JSX.Element {
    const { t } = useTranslation();

    return (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '16px 20px 20px' }}>
            <div className="automation-profile-groups">
                {/* Profile Name */}
                <div className="automation-profile-card">
                    <SettingsItem title={t('automation.profile_name', { defaultValue: 'Profile Name' })} layout="vertical">
                        <input
                            className="settings-input"
                            value={draft.name}
                            onChange={(event) => onChange({ ...draft, name: event.target.value })}
                            placeholder={t('automation.profile_name_placeholder', { defaultValue: 'e.g. Customer interviews' })}
                        />
                    </SettingsItem>
                </div>

                {/* Section 1: Language & AI Templates */}
                <div className="automation-profile-card">
                    <div className="automation-profile-card-title">
                        {t('automation.profile_section_ai', { defaultValue: '1. Language & AI Presets' })}
                    </div>
                    <div className="automation-step-content">
                        <SettingsItem title={t('settings.language', { defaultValue: 'Language' })}>
                            <Dropdown
                                value={draft.translationLanguage}
                                onChange={(translationLanguage) => onChange({ ...draft, translationLanguage })}
                                options={languageOptions}
                                style={{ width: '220px' }}
                                aria-label={t('settings.language', { defaultValue: 'Language' })}
                            />
                        </SettingsItem>
                        <SettingsItem title={t('projects.polish_preset', { defaultValue: 'Polish Preset' })}>
                            <Dropdown
                                value={draft.polishPresetId}
                                onChange={(polishPresetId) => onChange({ ...draft, polishPresetId })}
                                options={polishPresetOptions}
                                style={{ width: '220px' }}
                                aria-label={t('projects.polish_preset', { defaultValue: 'Polish Preset' })}
                            />
                        </SettingsItem>
                        <SettingsItem title={t('projects.summary_template', { defaultValue: 'Summary Template' })}>
                            <Dropdown
                                value={draft.summaryTemplateId}
                                onChange={(summaryTemplateId) => onChange({ ...draft, summaryTemplateId })}
                                options={summaryTemplateOptions}
                                style={{ width: '220px' }}
                                aria-label={t('projects.summary_template', { defaultValue: 'Summary Template' })}
                            />
                        </SettingsItem>
                    </div>
                </div>

                {/* Section 2: Vocabularies & Keyword Sets */}
                <div className="automation-profile-card">
                    <div className="automation-profile-card-title">
                        {t('automation.profile_section_vocab', { defaultValue: '2. Vocabularies & Hotwords' })}
                    </div>
                    <div className="automation-step-content">
                        <SetSelector
                            title={t('projects.text_replacement_sets', { defaultValue: 'Text Replacement Sets' })}
                            items={textReplacementSets}
                            selectedIds={draft.enabledTextReplacementSetIds}
                            onChange={(enabledTextReplacementSetIds) => onChange({ ...draft, enabledTextReplacementSetIds })}
                        />
                        <SetSelector
                            title={t('projects.hotword_sets', { defaultValue: 'Hotword Sets' })}
                            items={hotwordSets}
                            selectedIds={draft.enabledHotwordSetIds}
                            onChange={(enabledHotwordSetIds) => onChange({ ...draft, enabledHotwordSetIds })}
                        />
                        <SetSelector
                            title={t('projects.polish_keyword_sets', { defaultValue: 'Polish Keyword Sets' })}
                            items={polishKeywordSets}
                            selectedIds={draft.enabledPolishKeywordSetIds}
                            onChange={(enabledPolishKeywordSetIds) => onChange({ ...draft, enabledPolishKeywordSetIds })}
                        />
                    </div>
                </div>

                {/* Section 3: Speaker Profiles */}
                <div className="automation-profile-card">
                    <div className="automation-profile-card-title">
                        {t('automation.profile_section_speakers', { defaultValue: '3. Speaker Identification' })}
                    </div>
                    <div className="automation-step-content">
                        <SetSelector
                            title={t('projects.speaker_profiles', { defaultValue: 'Speaker Profiles' })}
                            items={speakerProfiles}
                            selectedIds={draft.enabledSpeakerProfileIds}
                            onChange={(enabledSpeakerProfileIds) => onChange({ ...draft, enabledSpeakerProfileIds })}
                        />
                    </div>
                </div>
            </div>

            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px', paddingTop: '8px' }}>
                <button className="btn" onClick={onCancel}>{t('common.cancel')}</button>
                <button className="btn btn-primary" onClick={onSave}>{t('common.save')}</button>
            </div>
        </div>
    );
}

export default AutomationProfileEditor;
