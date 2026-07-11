import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2 } from 'lucide-react';
import { v4 as uuidv4 } from 'uuid';
import { BookIcon } from '../Icons';
import type {
  HotwordRule,
  HotwordRuleSet,
  PolishKeywordRuleSet,
  TextReplacementRule,
  TextReplacementRuleSet,
} from '../../types/config';
import { useVocabularyConfig, useSetConfig } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import type { ProjectDefaults } from '../../types/project';
import { SettingsTabContainer, SettingsPageHeader } from './SettingsLayout';
import { Switch } from '../Switch';
import { SettingsContextSection } from './SettingsContextSection';
import { SettingsSummaryTemplateSection } from './SettingsSummaryTemplateSection';
import { SettingsSpeakerProfilesSection } from './SettingsSpeakerProfilesSection';
import { normalizePolishKeywordSets } from '../../utils/polishKeywords';
import { RuleSetSection } from './vocabulary/RuleSetSection';

type ProjectRuleSetDefaultsKey =
  | 'enabledTextReplacementSetIds'
  | 'enabledHotwordSetIds'
  | 'enabledPolishKeywordSetIds';

interface RuleSetUiState {
  newSetName: string;
  setNewSetName: React.Dispatch<React.SetStateAction<string>>;
  expandedSetIds: Set<string>;
  batchEditingSetIds: Set<string>;
  expandSet: (id: string) => void;
  toggleExpanded: (id: string) => void;
  toggleBatchEditing: (id: string) => void;
}

const TEXTAREA_STYLE: React.CSSProperties = {
  width: '100%',
  minHeight: '120px',
  fontFamily: 'var(--font-mono)',
  fontSize: '0.85rem',
  resize: 'vertical',
  lineHeight: '1.4',
  padding: '10px',
};

const HINT_STYLE: React.CSSProperties = {
  fontSize: '0.75rem',
  color: 'var(--color-text-muted)',
  margin: 0,
  marginTop: '4px',
};

const RULE_ROW_STYLE: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
  padding: '8px 12px',
  background: 'var(--color-bg-secondary)',
  borderRadius: 'var(--radius-md)',
};

const RULE_ROW_CONTENT_STYLE: React.CSSProperties = {
  flex: 1,
  display: 'flex',
  gap: '12px',
  alignItems: 'center',
};

const ADD_RULE_BUTTON_STYLE: React.CSSProperties = {
  alignSelf: 'flex-start',
  marginTop: '4px',
  fontSize: '0.85rem',
  display: 'flex',
  alignItems: 'center',
  gap: '6px',
  padding: '6px 16px',
};

function toggleId(current: Set<string>, id: string): Set<string> {
  const next = new Set(current);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  return next;
}

function useRuleSetUiState(): RuleSetUiState {
  const [newSetName, setNewSetName] = useState('');
  const [expandedSetIds, setExpandedSetIds] = useState<Set<string>>(new Set());
  const [batchEditingSetIds, setBatchEditingSetIds] = useState<Set<string>>(new Set());

  const expandSet = (id: string) => {
    setExpandedSetIds((current) => new Set(current).add(id));
  };

  const toggleExpanded = (id: string) => {
    setExpandedSetIds((current) => toggleId(current, id));
  };

  const toggleBatchEditing = (id: string) => {
    const isEnteringBatchMode = !batchEditingSetIds.has(id);
    setBatchEditingSetIds((current) => toggleId(current, id));
    if (isEnteringBatchMode) {
      expandSet(id);
    }
  };

  return {
    newSetName,
    setNewSetName,
    expandedSetIds,
    batchEditingSetIds,
    expandSet,
    toggleExpanded,
    toggleBatchEditing,
  };
}

function updateSetById<TSet extends { id: string }>(
  sets: TSet[],
  id: string,
  updates: Partial<TSet>,
): TSet[] {
  return sets.map((set) => (set.id === id ? { ...set, ...updates } : set));
}

function removeSetById<TSet extends { id: string }>(sets: TSet[], id: string): TSet[] {
  return sets.filter((set) => set.id !== id);
}

function updateRulesForSet<TSet extends { id: string; rules: unknown[] }>(
  sets: TSet[],
  setId: string,
  getNextRules: (rules: TSet['rules']) => TSet['rules'],
): TSet[] {
  return sets.map((set) => (
    set.id === setId ? { ...set, rules: getNextRules(set.rules) } as TSet : set
  ));
}

function updateRuleById<TRule extends { id: string }>(
  rules: TRule[],
  ruleId: string,
  updates: Partial<TRule>,
): TRule[] {
  return rules.map((rule) => (rule.id === ruleId ? { ...rule, ...updates } : rule));
}

function rulesToString(rules: TextReplacementRule[]): string {
  return rules.map((rule) => `${rule.from} => ${rule.to}`).join('\n');
}

function stringToRules(value: string): TextReplacementRule[] {
  return value.split('\n')
    .filter((line) => line.trim() !== '')
    .map((line) => {
      const separators = ['=>', '->', '=', ':'] as const;
      for (const separator of separators) {
        if (line.includes(separator)) {
          const [fromPart = '', toPart = ''] = line.split(separator);
          return { id: uuidv4(), from: fromPart.trim(), to: toPart.trim() };
        }
      }

      return { id: uuidv4(), from: line.trim(), to: '' };
    });
}

function hotwordsToString(rules: HotwordRule[]): string {
  return rules.map((rule) => rule.text).join('\n');
}

function stringToHotwords(value: string): HotwordRule[] {
  return value.split('\n')
    .filter((line) => line.trim() !== '')
    .map((line) => ({ id: uuidv4(), text: line.trim() }));
}

export function SettingsVocabularyTab(): React.JSX.Element {
  const { t } = useTranslation();
  const config = useVocabularyConfig();
  const updateConfig = useSetConfig();
  const projects = useProjectStore((state) => state.projects);
  const updateProjectDefaults = useProjectStore((state) => state.updateProjectDefaults);

  const textReplacementUi = useRuleSetUiState();
  const hotwordUi = useRuleSetUiState();
  const polishKeywordUi = useRuleSetUiState();

  const sets = config.textReplacementSets || [];
  const hotwordSets = config.hotwordSets || [];
  const polishKeywordSets = normalizePolishKeywordSets(config.polishKeywordSets);

  const removeRuleSetReferenceFromProjects = async (
    key: ProjectRuleSetDefaultsKey,
    setId: string,
  ) => {
    const affectedProjects = projects.filter((project) => project.defaults[key].includes(setId));
    if (affectedProjects.length === 0) {
      return;
    }

    await Promise.all(affectedProjects.map((project) => (
      updateProjectDefaults(project.id, {
        [key]: project.defaults[key].filter((id) => id !== setId),
      } as Pick<ProjectDefaults, typeof key>)
    )));
  };

  const handleAddSet = () => {
    const setName = textReplacementUi.newSetName.trim();
    if (!setName) return;

    const newSet: TextReplacementRuleSet = {
      id: uuidv4(),
      name: setName,
      enabled: true,
      ignoreCase: false,
      rules: [],
    };

    updateConfig({ textReplacementSets: [...sets, newSet] });
    textReplacementUi.setNewSetName('');
    textReplacementUi.expandSet(newSet.id);
  };

  const handleUpdateSet = (id: string, updates: Partial<TextReplacementRuleSet>) => {
    updateConfig({ textReplacementSets: updateSetById(sets, id, updates) });
  };

  const handleDeleteSet = async (id: string) => {
    updateConfig({ textReplacementSets: removeSetById(sets, id) });
    await removeRuleSetReferenceFromProjects('enabledTextReplacementSetIds', id);
  };

  const handleAddRuleToSet = (setId: string) => {
    updateConfig({
      textReplacementSets: updateRulesForSet(sets, setId, (rules) => [
        ...rules,
        { id: uuidv4(), from: '', to: '' },
      ]),
    });
  };

  const handleUpdateRuleInSet = (
    setId: string,
    ruleId: string,
    updates: Partial<TextReplacementRule>,
  ) => {
    updateConfig({
      textReplacementSets: updateRulesForSet(sets, setId, (rules) => updateRuleById(rules, ruleId, updates)),
    });
  };

  const handleDeleteRuleFromSet = (setId: string, ruleId: string) => {
    updateConfig({
      textReplacementSets: updateRulesForSet(
        sets,
        setId,
        (rules) => rules.filter((rule) => rule.id !== ruleId),
      ),
    });
  };

  const handleAddHotwordSet = () => {
    const setName = hotwordUi.newSetName.trim();
    if (!setName) return;

    const newSet: HotwordRuleSet = {
      id: uuidv4(),
      name: setName,
      enabled: true,
      rules: [],
    };

    updateConfig({ hotwordSets: [...hotwordSets, newSet] });
    hotwordUi.setNewSetName('');
    hotwordUi.expandSet(newSet.id);
  };

  const handleUpdateHotwordSet = (id: string, updates: Partial<HotwordRuleSet>) => {
    updateConfig({ hotwordSets: updateSetById(hotwordSets, id, updates) });
  };

  const handleDeleteHotwordSet = async (id: string) => {
    updateConfig({ hotwordSets: removeSetById(hotwordSets, id) });
    await removeRuleSetReferenceFromProjects('enabledHotwordSetIds', id);
  };

  const handleAddHotwordToSet = (setId: string) => {
    updateConfig({
      hotwordSets: updateRulesForSet(hotwordSets, setId, (rules) => [
        ...rules,
        { id: uuidv4(), text: '' },
      ]),
    });
  };

  const handleUpdateHotwordInSet = (
    setId: string,
    ruleId: string,
    updates: Partial<HotwordRule>,
  ) => {
    updateConfig({
      hotwordSets: updateRulesForSet(hotwordSets, setId, (rules) => updateRuleById(rules, ruleId, updates)),
    });
  };

  const handleDeleteHotwordFromSet = (setId: string, ruleId: string) => {
    updateConfig({
      hotwordSets: updateRulesForSet(
        hotwordSets,
        setId,
        (rules) => rules.filter((rule) => rule.id !== ruleId),
      ),
    });
  };

  const handleAddPolishKeywordSet = () => {
    const setName = polishKeywordUi.newSetName.trim();
    if (!setName) return;

    const newSet: PolishKeywordRuleSet = {
      id: uuidv4(),
      name: setName,
      enabled: true,
      keywords: '',
    };

    updateConfig({ polishKeywordSets: [...polishKeywordSets, newSet] });
    polishKeywordUi.setNewSetName('');
    polishKeywordUi.expandSet(newSet.id);
  };

  const handleUpdatePolishKeywordSet = (id: string, updates: Partial<PolishKeywordRuleSet>) => {
    updateConfig({ polishKeywordSets: updateSetById(polishKeywordSets, id, updates) });
  };

  const handleDeletePolishKeywordSet = async (id: string) => {
    updateConfig({ polishKeywordSets: removeSetById(polishKeywordSets, id) });
    await removeRuleSetReferenceFromProjects('enabledPolishKeywordSetIds', id);
  };

  return (
    <SettingsTabContainer id="settings-panel-vocabulary" ariaLabelledby="settings-tab-vocabulary">
      <SettingsPageHeader
        icon={<BookIcon width={28} height={28} />}
        title={t('settings.vocabulary')}
        description={t('settings.vocabulary_description', { defaultValue: 'Manage custom vocabulary, hotwords, polish keyword sets, text polish context presets, and summary templates.' })}
      />

      <RuleSetSection
        title={t('settings.text_replacement_title', { defaultValue: 'Text Replacement' })}
        icon={<BookIcon width={20} height={20} />}
        description={t('settings.text_replacement_description', { defaultValue: 'Group rules into sets to easily enable or disable them.' })}
        sets={sets}
        newSetName={textReplacementUi.newSetName}
        newSetPlaceholder={t('settings.rule_set_name_placeholder', { defaultValue: 'e.g. Technical Terms' })}
        emptyLabel={t('settings.no_rule_sets', { defaultValue: 'No rule sets defined.' })}
        expandedSetIds={textReplacementUi.expandedSetIds}
        onAddSet={handleAddSet}
        onDeleteSet={handleDeleteSet}
        onNewSetNameChange={textReplacementUi.setNewSetName}
        onToggleEnabled={(id, enabled) => handleUpdateSet(id, { enabled })}
        onToggleExpanded={textReplacementUi.toggleExpanded}
        onUpdateSetName={(id, name) => handleUpdateSet(id, { name })}
        renderBadge={(set) => (
          <>
            {set.rules.length} {t('settings.rules_count', { count: set.rules.length, defaultValue: 'rules' })}
          </>
        )}
        getBatchToggle={(set) => ({
          isEditing: textReplacementUi.batchEditingSetIds.has(set.id),
          onToggle: () => textReplacementUi.toggleBatchEditing(set.id),
        })}
        renderExtraControls={(set) => (
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <Switch
              checked={set.ignoreCase}
              onChange={(checked) => handleUpdateSet(set.id, { ignoreCase: checked })}
              style={{ transform: 'scale(0.8)' }}
            />
            <span style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', fontWeight: 500 }}>
              {t('settings.ignore_case')}
            </span>
          </div>
        )}
        renderExpanded={(set) => (
          textReplacementUi.batchEditingSetIds.has(set.id) ? (
            <>
              <textarea
                className="settings-input"
                value={rulesToString(set.rules)}
                onChange={(event) => handleUpdateSet(set.id, { rules: stringToRules(event.target.value) })}
                placeholder={t('settings.rules_placeholder', { defaultValue: 'e.g. Find => Replace With' })}
                rows={5}
                style={TEXTAREA_STYLE}
              />
              <p style={HINT_STYLE}>
                {t('settings.rules_hint', { defaultValue: 'Use " => " to separate find and replace text. One rule per line.' })}
              </p>
            </>
          ) : (
            <>
              {set.rules.map((rule) => (
                <div key={rule.id} style={RULE_ROW_STYLE}>
                  <div style={RULE_ROW_CONTENT_STYLE}>
                    <input
                      type="text"
                      className="settings-input-minimal"
                      value={rule.from}
                      onChange={(event) => handleUpdateRuleInSet(set.id, rule.id, { from: event.target.value })}
                      placeholder={t('settings.find')}
                      style={{ fontWeight: 500 }}
                    />
                    <div style={{ color: 'var(--color-text-muted)', fontSize: '0.8rem', opacity: 0.6 }}>{'=>'}</div>
                    <input
                      type="text"
                      className="settings-input-minimal"
                      value={rule.to}
                      onChange={(event) => handleUpdateRuleInSet(set.id, rule.id, { to: event.target.value })}
                      placeholder={t('settings.replace_with')}
                    />
                  </div>
                  <button
                    className="btn btn-icon btn-danger-soft"
                    onClick={() => handleDeleteRuleFromSet(set.id, rule.id)}
                    style={{ padding: '4px' }}
                    aria-label={t('common.delete')}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              ))}

              <button
                className="btn btn-secondary-soft"
                onClick={() => handleAddRuleToSet(set.id)}
                style={ADD_RULE_BUTTON_STYLE}
              >
                <Plus size={16} />
                {t('common.add')}
              </button>
            </>
          )
        )}
      />

      <RuleSetSection
        title={t('settings.hotwords_title', { defaultValue: 'Hotwords' })}
        icon={<BookIcon width={20} height={20} />}
        description={t('settings.hotwords_description', { defaultValue: 'Enhance recognition for specific terms. One per line. (Supported by Transducer and Qwen3 models)' })}
        sets={hotwordSets}
        newSetName={hotwordUi.newSetName}
        newSetPlaceholder={t('settings.rule_set_name_placeholder', { defaultValue: 'e.g. Technical Terms' })}
        emptyLabel={t('settings.no_rule_sets', { defaultValue: 'No rule sets defined.' })}
        expandedSetIds={hotwordUi.expandedSetIds}
        onAddSet={handleAddHotwordSet}
        onDeleteSet={handleDeleteHotwordSet}
        onNewSetNameChange={hotwordUi.setNewSetName}
        onToggleEnabled={(id, enabled) => handleUpdateHotwordSet(id, { enabled })}
        onToggleExpanded={hotwordUi.toggleExpanded}
        onUpdateSetName={(id, name) => handleUpdateHotwordSet(id, { name })}
        renderBadge={(set) => (
          <>
            {set.rules.length} {t('settings.rules_count', { count: set.rules.length, defaultValue: 'rules' })}
          </>
        )}
        getBatchToggle={(set) => ({
          isEditing: hotwordUi.batchEditingSetIds.has(set.id),
          onToggle: () => hotwordUi.toggleBatchEditing(set.id),
        })}
        renderExpanded={(set) => (
          hotwordUi.batchEditingSetIds.has(set.id) ? (
            <>
              <textarea
                className="settings-input"
                value={hotwordsToString(set.rules)}
                onChange={(event) => handleUpdateHotwordSet(set.id, { rules: stringToHotwords(event.target.value) })}
                placeholder={t('settings.hotwords_placeholder', { defaultValue: 'e.g. ChatGPT\nSherpa-onnx :2.0' })}
                rows={5}
                style={TEXTAREA_STYLE}
              />
              <p style={HINT_STYLE}>
                {t('settings.hotwords_hint', { defaultValue: 'Tip: You can add weight by appending " :weight" (e.g. "Term :2.0"). Default weight is 1.0.' })}
              </p>
            </>
          ) : (
            <>
              {set.rules.map((rule) => (
                <div key={rule.id} style={RULE_ROW_STYLE}>
                  <div style={RULE_ROW_CONTENT_STYLE}>
                    <input
                      type="text"
                      className="settings-input-minimal"
                      value={rule.text}
                      onChange={(event) => handleUpdateHotwordInSet(set.id, rule.id, { text: event.target.value })}
                      placeholder={t('settings.hotwords_placeholder', { defaultValue: 'e.g. ChatGPT' })}
                      style={{ fontWeight: 500 }}
                    />
                  </div>
                  <button
                    className="btn btn-icon btn-danger-soft"
                    onClick={() => handleDeleteHotwordFromSet(set.id, rule.id)}
                    style={{ padding: '4px' }}
                    aria-label={t('common.delete')}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              ))}

              <button
                className="btn btn-secondary-soft"
                onClick={() => handleAddHotwordToSet(set.id)}
                style={ADD_RULE_BUTTON_STYLE}
              >
                <Plus size={16} />
                {t('common.add')}
              </button>
              <p style={HINT_STYLE}>
                {t('settings.hotwords_hint', { defaultValue: 'Tip: You can add weight by appending " :weight" (e.g. "Term :2.0"). Default weight is 1.0.' })}
              </p>
            </>
          )
        )}
      />

      <RuleSetSection
        title={t('settings.polish_keywords_title', { defaultValue: 'Polish Keywords' })}
        icon={<BookIcon width={20} height={20} />}
        description={t('settings.polish_keywords_description', {
          defaultValue: 'Group reusable keyword guidance into global sets. Enabled sets are combined when text polish runs.',
        })}
        sets={polishKeywordSets}
        newSetName={polishKeywordUi.newSetName}
        newSetPlaceholder={t('settings.polish_keyword_set_name_placeholder', { defaultValue: 'e.g. Brand Terms' })}
        emptyLabel={t('settings.no_polish_keyword_sets', { defaultValue: 'No polish keyword sets yet.' })}
        expandedSetIds={polishKeywordUi.expandedSetIds}
        onAddSet={handleAddPolishKeywordSet}
        onDeleteSet={handleDeletePolishKeywordSet}
        onNewSetNameChange={polishKeywordUi.setNewSetName}
        onToggleEnabled={(id, enabled) => handleUpdatePolishKeywordSet(id, { enabled })}
        onToggleExpanded={polishKeywordUi.toggleExpanded}
        onUpdateSetName={(id, name) => handleUpdatePolishKeywordSet(id, { name })}
        renderBadge={(set) => (
          set.keywords.trim()
            ? t('settings.polish_keywords_ready', { defaultValue: 'Ready' })
            : t('settings.polish_keywords_empty', { defaultValue: 'Empty' })
        )}
        renderExpanded={(set) => (
          <>
            <textarea
              className="settings-input"
              value={set.keywords}
              onChange={(event) => handleUpdatePolishKeywordSet(set.id, { keywords: event.target.value })}
              placeholder={t('settings.polish_keywords_placeholder', {
                defaultValue: 'e.g. Product names, terminology, preferred spellings',
              })}
              rows={5}
              style={TEXTAREA_STYLE}
            />
            <p style={HINT_STYLE}>
              {t('settings.polish_keywords_hint', {
                defaultValue: 'Use this block for preferred terms or style guidance. Enabled sets are combined in order during polishing.',
              })}
            </p>
          </>
        )}
      />

      <SettingsSpeakerProfilesSection />
      <SettingsContextSection />
      <SettingsSummaryTemplateSection />
    </SettingsTabContainer>
  );
}
