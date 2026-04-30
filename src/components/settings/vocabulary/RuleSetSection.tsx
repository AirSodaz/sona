import React from 'react';
import { ChevronDown, ChevronRight, FileText, List, Plus, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Switch } from '../../Switch';
import { SettingsSection } from '../SettingsLayout';

export interface RuleSetBase {
  id: string;
  name: string;
  enabled: boolean;
}

export interface BatchToggleConfig {
  isEditing: boolean;
  onToggle: () => void;
}

interface RuleSetSectionProps<TSet extends RuleSetBase> {
  title: string;
  icon: React.ReactNode;
  description: string;
  sets: TSet[];
  newSetName: string;
  newSetPlaceholder: string;
  emptyLabel: React.ReactNode;
  expandedSetIds: Set<string>;
  onAddSet: () => void;
  onDeleteSet: (id: string) => void;
  onNewSetNameChange: (name: string) => void;
  onToggleEnabled: (id: string, enabled: boolean) => void;
  onToggleExpanded: (id: string) => void;
  onUpdateSetName: (id: string, name: string) => void;
  renderBadge: (set: TSet) => React.ReactNode;
  renderExpanded: (set: TSet) => React.ReactNode;
  getBatchToggle?: (set: TSet) => BatchToggleConfig;
  renderExtraControls?: (set: TSet) => React.ReactNode;
}

export function RuleSetSection<TSet extends RuleSetBase>({
  title,
  icon,
  description,
  sets,
  newSetName,
  newSetPlaceholder,
  emptyLabel,
  expandedSetIds,
  onAddSet,
  onDeleteSet,
  onNewSetNameChange,
  onToggleEnabled,
  onToggleExpanded,
  onUpdateSetName,
  renderBadge,
  renderExpanded,
  getBatchToggle,
  renderExtraControls,
}: RuleSetSectionProps<TSet>): React.JSX.Element {
  const { t } = useTranslation();

  return (
    <SettingsSection title={title} icon={icon} description={description}>
      <div style={{
        display: 'flex',
        gap: '12px',
        padding: '24px',
        background: 'var(--color-bg-primary)',
        alignItems: 'flex-end',
        borderBottom: '1px solid var(--color-border-subtle)'
      }}>
        <div style={{ flex: 1 }}>
          <label style={{ display: 'block', fontSize: '0.85rem', marginBottom: '4px', color: 'var(--color-text-muted)' }}>
            {t('settings.rule_set_name', { defaultValue: 'Rule Set Name' })}
          </label>
          <input
            type="text"
            className="settings-input"
            value={newSetName}
            onChange={(event) => onNewSetNameChange(event.target.value)}
            placeholder={newSetPlaceholder}
            style={{ width: '100%' }}
          />
        </div>
        <button
          className="btn btn-primary"
          onClick={onAddSet}
          disabled={!newSetName.trim()}
          style={{ height: '38px', display: 'flex', alignItems: 'center', gap: '6px', padding: '0 20px' }}
        >
          <Plus size={18} />
          {t('settings.add_rule_set', { defaultValue: 'Add Set' })}
        </button>
      </div>

      <div className="settings-list" style={{ background: 'var(--color-bg-primary)', overflow: 'hidden' }}>
        {sets.length === 0 ? (
          <div style={{
            padding: '48px 24px',
            textAlign: 'center',
            color: 'var(--color-text-muted)'
          }}>
            {emptyLabel}
          </div>
        ) : (
          sets.map((set, index) => {
            const isExpanded = expandedSetIds.has(set.id);
            const batchToggle = getBatchToggle?.(set);
            const batchToggleLabel = batchToggle
              ? batchToggle.isEditing
                ? t('settings.switch_to_list', { defaultValue: 'Switch to List' })
                : t('settings.switch_to_text', { defaultValue: 'Switch to Text' })
              : undefined;

            return (
              <div key={set.id} style={{
                borderBottom: index === sets.length - 1 ? 'none' : '1px solid var(--color-border-subtle)',
                background: set.enabled ? 'transparent' : 'var(--color-bg-secondary-soft)',
              }}>
                <div style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '12px',
                  padding: '16px 24px',
                  cursor: 'pointer'
                }} onClick={() => onToggleExpanded(set.id)}>
                  <div style={{ display: 'flex', alignItems: 'center', color: 'var(--color-text-muted)' }}>
                    {isExpanded ? <ChevronDown size={20} /> : <ChevronRight size={20} />}
                  </div>

                  <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <input
                      type="text"
                      className="settings-input-minimal"
                      value={set.name}
                      onClick={(event) => event.stopPropagation()}
                      onChange={(event) => onUpdateSetName(set.id, event.target.value)}
                      style={{ fontWeight: 600, fontSize: '1rem', width: 'auto', minWidth: '150px' }}
                    />
                    <span style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', background: 'var(--color-bg-secondary)', padding: '2px 8px', borderRadius: 'var(--radius-sm)' }}>
                      {renderBadge(set)}
                    </span>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '20px' }} onClick={(event) => event.stopPropagation()}>
                    {batchToggle && (
                      <button
                        className="btn btn-icon btn-secondary-soft"
                        onClick={batchToggle.onToggle}
                        title={batchToggleLabel}
                        aria-label={batchToggleLabel}
                      >
                        {batchToggle.isEditing ? <List size={18} /> : <FileText size={18} />}
                      </button>
                    )}

                    {renderExtraControls?.(set)}

                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <Switch
                        checked={set.enabled}
                        onChange={(checked) => onToggleEnabled(set.id, checked)}
                      />
                    </div>

                    <button
                      className="btn btn-icon btn-danger-soft"
                      onClick={() => onDeleteSet(set.id)}
                      title={t('settings.delete_rule_set', { defaultValue: `Delete ${set.name}` })}
                      aria-label={t('settings.delete_rule_set', { defaultValue: `Delete ${set.name}` })}
                    >
                      <Trash2 size={18} />
                    </button>
                  </div>
                </div>

                {isExpanded && (
                  <div style={{
                    padding: '0 24px 24px 56px',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '8px'
                  }}>
                    {renderExpanded(set)}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </SettingsSection>
  );
}
