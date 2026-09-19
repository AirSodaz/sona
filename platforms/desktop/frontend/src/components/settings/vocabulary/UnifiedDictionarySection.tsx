import {
  AlertCircle,
  AlertTriangle,
  ArrowRight,
  Book,
  Check,
  ChevronDown,
  Code2,
  Copy,
  Eye,
  HelpCircle,
  Pencil,
  Plus,
  Search,
  Sparkles,
  Trash2,
  Wand2,
  X,
} from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useProjectStore } from '../../../stores/projectStore';
import type { HotwordRuleSet, TextReplacementRuleSet } from '../../../types/config';
import {
  addTermToYamlDictionary,
  extractHotwordWeight,
  formatHotwordWithWeight,
  formatYamlDictionary,
  isVoiceTypingScope,
  parseYamlDictionary,
  removeTermFromYamlDictionary,
  serializeToYamlDictionary,
  updateTermInYamlDictionary,
  VOICE_TYPING_SCOPE_ID,
  validateYamlDictionary,
} from '../../../utils/yamlDictionaryParser';
import { Dropdown } from '../../Dropdown';
import { Modal } from '../../Modal';
import { SettingsSection } from '../SettingsLayout';
import './vocabulary.css';

interface UnifiedDictionarySectionProps {
  content?: string;
  defaultMode?: 'visual' | 'code';
  onUpdateContent: (content: string) => void;
  hotwordSets?: HotwordRuleSet[];
  textReplacementSets?: TextReplacementRuleSet[];
  onUpdateHotwordSets?: (sets: HotwordRuleSet[]) => void;
  onUpdateTextReplacementSets?: (sets: TextReplacementRuleSet[]) => void;
}

export interface VisualTermItem {
  id: string;
  scopeId: string;
  scopeName: string;
  isReplacement: boolean;
  text: string;
  from?: string;
  to?: string;
  weight?: number;
  line: number;
  raw: string;
}

export function UnifiedDictionarySection({
  content = '',
  defaultMode = 'visual',
  onUpdateContent,
  hotwordSets = [],
  textReplacementSets = [],
  onUpdateHotwordSets,
  onUpdateTextReplacementSets,
}: UnifiedDictionarySectionProps): React.JSX.Element {
  const { t } = useTranslation();
  const projects = useProjectStore((state) => state.projects);

  // Initialize content from legacy sets if empty
  const initialContent = useMemo(() => {
    if (content.trim()) {
      return content;
    }
    return serializeToYamlDictionary(hotwordSets, textReplacementSets, projects);
  }, [content, hotwordSets, textReplacementSets, projects]);

  const [text, setText] = useState(initialContent);
  const [mode, setMode] = useState<'visual' | 'code'>(defaultMode);
  const [scopeFilter, setScopeFilter] = useState<string>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [inlineHotwordInput, setInlineHotwordInput] = useState('');

  // Quick Add (available at top)
  const [quickAddInput, setQuickAddInput] = useState('');
  const [quickAddTarget, setQuickAddTarget] = useState<string>('global');

  // Modal State (Add / Edit)
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [editingItem, setEditingItem] = useState<VisualTermItem | null>(null);
  const [formType, setFormType] = useState<'hotword' | 'replacement'>('hotword');
  const [formScope, setFormScope] = useState<string>('global');
  const [formWord, setFormWord] = useState('');
  const [formWeight, setFormWeight] = useState('');
  const [formFrom, setFormFrom] = useState('');
  const [formTo, setFormTo] = useState('');

  // Test Tool State
  const [testInput, setTestInput] = useState('');

  // Feedbacks & Editor State
  const [copyFeedback, setCopyFeedback] = useState(false);
  const [formatFeedback, setFormatFeedback] = useState(false);
  const [cursorLine, setCursorLine] = useState(1);
  const [suggestions, setSuggestions] = useState<Array<{ id: string; name: string }>>([]);
  const [suggestionPos, setSuggestionPos] = useState<{ top: number; left: number } | null>(null);
  const [isTutorialOpen, setIsTutorialOpen] = useState(false);

  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  // Keep local text in sync when incoming content changes externally
  useEffect(() => {
    if (content.trim() && content !== text) {
      setText(content);
    }
  }, [content, text]);

  // Sync changes up to parent and legacy stores
  const syncChanges = useCallback(
    (newText: string) => {
      setText(newText);
      onUpdateContent(newText);

      // Parse and sync to legacy sets for backwards compatibility
      if (onUpdateHotwordSets || onUpdateTextReplacementSets) {
        const parsed = parseYamlDictionary(newText, projects);

        if (onUpdateHotwordSets) {
          const globalHotwords = parsed.globalTerms
            .filter((term) => !term.isReplacement && term.text)
            .map((term, idx) => ({ id: `hw_${idx}`, text: term.text }));
          onUpdateHotwordSets([
            {
              id: 'unified-hotwords',
              name: 'Unified Hotwords',
              enabled: true,
              rules: globalHotwords,
            },
          ]);
        }

        if (onUpdateTextReplacementSets) {
          const globalReplacements = parsed.globalTerms
            .filter((term) => term.isReplacement && term.from && term.to)
            .map((term, idx) => ({
              id: `rep_${idx}`,
              from: term.from!,
              to: term.to!,
            }));
          onUpdateTextReplacementSets([
            {
              id: 'unified-replacements',
              name: 'Unified Replacements',
              enabled: true,
              ignoreCase: false,
              rules: globalReplacements,
            },
          ]);
        }
      }
    },
    [onUpdateContent, onUpdateHotwordSets, onUpdateTextReplacementSets, projects]
  );

  // Diagnostics
  const diagnostics = useMemo(() => {
    return validateYamlDictionary(text, projects);
  }, [text, projects]);

  const errorCount = diagnostics.filter((d) => d.severity === 'error').length;

  // Parsed representation
  const parsed = useMemo(() => {
    return parseYamlDictionary(text, projects);
  }, [text, projects]);

  const totalTermsCount =
    parsed.globalTerms.length + parsed.projects.reduce((sum, p) => sum + p.terms.length, 0);

  // Flattened Visual Terms
  const visualItems = useMemo<VisualTermItem[]>(() => {
    const items: VisualTermItem[] = [];

    // 1. Global terms
    for (let idx = 0; idx < parsed.globalTerms.length; idx++) {
      const term = parsed.globalTerms[idx];
      const weightInfo = term.isReplacement ? undefined : extractHotwordWeight(term.text);
      items.push({
        id: `global-${term.line}-${idx}`,
        scopeId: 'global',
        scopeName: t('settings.dict_scope_badge_global', { defaultValue: 'Global' }),
        isReplacement: term.isReplacement,
        text: weightInfo ? weightInfo.word : term.text,
        weight: weightInfo?.weight,
        from: term.from,
        to: term.to,
        line: term.line,
        raw: term.raw,
      });
    }

    // 2. Project sections
    for (const proj of parsed.projects) {
      const isVT = isVoiceTypingScope(proj.projectId, proj.projectName);
      const scopeId = isVT ? VOICE_TYPING_SCOPE_ID : proj.projectId || proj.projectName;
      const scopeName = isVT
        ? t('settings.dict_scope_badge_vt', { defaultValue: 'Voice Typing' })
        : proj.projectName;

      for (let idx = 0; idx < proj.terms.length; idx++) {
        const term = proj.terms[idx];
        const weightInfo = term.isReplacement ? undefined : extractHotwordWeight(term.text);
        items.push({
          id: `${scopeId}-${term.line}-${idx}`,
          scopeId,
          scopeName,
          isReplacement: term.isReplacement,
          text: weightInfo ? weightInfo.word : term.text,
          weight: weightInfo?.weight,
          from: term.from,
          to: term.to,
          line: term.line,
          raw: term.raw,
        });
      }
    }

    return items;
  }, [parsed, t]);

  // Filtered visual terms based on scope and search
  const filteredItems = useMemo(() => {
    return visualItems.filter((item) => {
      if (scopeFilter !== 'all') {
        if (scopeFilter === 'global' && item.scopeId !== 'global') return false;
        if (scopeFilter === VOICE_TYPING_SCOPE_ID && item.scopeId !== VOICE_TYPING_SCOPE_ID)
          return false;
        if (
          scopeFilter !== 'global' &&
          scopeFilter !== VOICE_TYPING_SCOPE_ID &&
          item.scopeId !== scopeFilter
        ) {
          return false;
        }
      }

      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase().trim();
        if (item.isReplacement) {
          return Boolean(
            item.from?.toLowerCase().includes(q) || item.to?.toLowerCase().includes(q)
          );
        }
        return item.text.toLowerCase().includes(q);
      }

      return true;
    });
  }, [visualItems, scopeFilter, searchQuery]);

  const hotwordsList = useMemo(() => {
    return filteredItems.filter((item) => !item.isReplacement);
  }, [filteredItems]);

  const replacementsList = useMemo(() => {
    return filteredItems.filter((item) => item.isReplacement);
  }, [filteredItems]);

  // Scope counts for tab badges
  const scopeCounts = useMemo(() => {
    const counts: Record<string, number> = {
      all: visualItems.length,
      global: 0,
      [VOICE_TYPING_SCOPE_ID]: 0,
    };
    for (const p of projects) {
      counts[p.id] = 0;
    }
    for (const item of visualItems) {
      if (item.scopeId === 'global') counts.global++;
      else if (item.scopeId === VOICE_TYPING_SCOPE_ID) counts[VOICE_TYPING_SCOPE_ID]++;
      else if (counts[item.scopeId] !== undefined) counts[item.scopeId]++;
    }
    return counts;
  }, [visualItems, projects]);

  // Test output computation
  const testOutput = useMemo(() => {
    if (!testInput) return '';
    let result = testInput;
    for (const rep of replacementsList) {
      if (rep.from && rep.to) {
        result = result.split(rep.from).join(rep.to);
      }
    }
    return result;
  }, [testInput, replacementsList]);

  // Actions
  const handleFormat = () => {
    const formatted = formatYamlDictionary(text, projects);
    syncChanges(formatted);
    setFormatFeedback(true);
    setTimeout(() => setFormatFeedback(false), 1500);
  };

  const handleCopyAll = () => {
    void navigator.clipboard.writeText(text).then(() => {
      setCopyFeedback(true);
      setTimeout(() => setCopyFeedback(false), 1500);
    });
  };

  const handleQuickAdd = () => {
    const raw = quickAddInput.trim();
    if (!raw) return;

    let targetScope: 'global' | { id: string; name: string } = 'global';
    if (quickAddTarget === VOICE_TYPING_SCOPE_ID) {
      targetScope = {
        id: VOICE_TYPING_SCOPE_ID,
        name: t('settings.voice_typing', { defaultValue: 'Voice Typing' }),
      };
    } else if (quickAddTarget !== 'global') {
      const targetProj = projects.find((p) => p.id === quickAddTarget);
      if (targetProj) {
        targetScope = { id: targetProj.id, name: targetProj.name };
      }
    }

    const updated = addTermToYamlDictionary(text, raw, targetScope, projects);
    syncChanges(updated);
    setQuickAddInput('');
  };

  const handleInlineAddHotword = () => {
    const raw = inlineHotwordInput.trim();
    if (!raw) return;

    const targetScopeId = scopeFilter === 'all' ? 'global' : scopeFilter;
    let targetScope: 'global' | { id: string; name: string } = 'global';
    if (targetScopeId === VOICE_TYPING_SCOPE_ID) {
      targetScope = {
        id: VOICE_TYPING_SCOPE_ID,
        name: t('settings.voice_typing', { defaultValue: 'Voice Typing' }),
      };
    } else if (targetScopeId !== 'global') {
      const targetProj = projects.find((p) => p.id === targetScopeId);
      if (targetProj) {
        targetScope = { id: targetProj.id, name: targetProj.name };
      }
    }

    const updated = addTermToYamlDictionary(text, raw, targetScope, projects);
    syncChanges(updated);
    setInlineHotwordInput('');
  };

  const handleDeleteTerm = (item: VisualTermItem) => {
    const updated = removeTermFromYamlDictionary(
      text,
      {
        text: item.raw ? undefined : item.text,
        from: item.from,
        to: item.to,
        isReplacement: item.isReplacement,
        line: item.line,
        raw: item.raw,
      },
      item.scopeId,
      projects
    );
    syncChanges(updated);
  };

  const openNewModal = () => {
    setEditingItem(null);
    setFormType('hotword');
    setFormScope(scopeFilter === 'all' ? 'global' : scopeFilter);
    setFormWord('');
    setFormWeight('');
    setFormFrom('');
    setFormTo('');
    setIsAddModalOpen(true);
  };

  const openEditModal = (item: VisualTermItem) => {
    setEditingItem(item);
    setFormType(item.isReplacement ? 'replacement' : 'hotword');
    setFormScope(item.scopeId);
    if (item.isReplacement) {
      setFormFrom(item.from || '');
      setFormTo(item.to || '');
      setFormWord('');
      setFormWeight('');
    } else {
      setFormWord(item.text);
      setFormWeight(item.weight ? String(item.weight) : '');
      setFormFrom('');
      setFormTo('');
    }
    setIsAddModalOpen(true);
  };

  const handleModalSubmit = (e?: React.FormEvent) => {
    if (e) e.preventDefault();

    let targetScope: 'global' | { id: string; name: string } = 'global';
    if (formScope === VOICE_TYPING_SCOPE_ID) {
      targetScope = {
        id: VOICE_TYPING_SCOPE_ID,
        name: t('settings.voice_typing', { defaultValue: 'Voice Typing' }),
      };
    } else if (formScope !== 'global') {
      const targetProj = projects.find((p) => p.id === formScope);
      if (targetProj) {
        targetScope = { id: targetProj.id, name: targetProj.name };
      }
    }

    let formatted = '';
    if (formType === 'hotword') {
      const w = formWord.trim();
      if (!w) return;
      formatted = formatHotwordWithWeight(w, formWeight);
    } else {
      const f = formFrom.trim();
      const to = formTo.trim();
      if (!f || !to) return;
      formatted = `${f} -> ${to}`;
    }

    if (editingItem) {
      if (editingItem.scopeId !== formScope) {
        const withoutOld = removeTermFromYamlDictionary(
          text,
          editingItem,
          editingItem.scopeId,
          projects
        );
        const withNew = addTermToYamlDictionary(withoutOld, formatted, targetScope, projects);
        syncChanges(withNew);
      } else {
        const updated = updateTermInYamlDictionary(
          text,
          editingItem,
          formatted,
          targetScope,
          projects
        );
        syncChanges(updated);
      }
    } else {
      const updated = addTermToYamlDictionary(text, formatted, targetScope, projects);
      syncChanges(updated);
    }

    setIsAddModalOpen(false);
    setEditingItem(null);
  };

  // Jump to error line
  const jumpToLine = (lineNumber: number) => {
    if (!textareaRef.current) return;
    const lines = text.split('\n');
    let charIndex = 0;
    for (let i = 0; i < lineNumber - 1 && i < lines.length; i++) {
      charIndex += lines[i].length + 1;
    }
    textareaRef.current.focus();
    textareaRef.current.setSelectionRange(
      charIndex,
      charIndex + (lines[lineNumber - 1]?.length || 0)
    );
  };

  // Cursor tracking & Autocomplete
  const handleTextareaChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    syncChanges(val);

    const rawPos = e.target.selectionStart;
    const pos = rawPos !== null && rawPos !== undefined && rawPos > 0 ? rawPos : val.length;
    const linesUpToCursor = val.slice(0, pos).split('\n');
    const currentLineNum = linesUpToCursor.length;
    setCursorLine(currentLineNum);

    const currentLine = linesUpToCursor[linesUpToCursor.length - 1];
    const trimmedLine = currentLine.trim();

    const lines = val.split('\n');
    const projectsIndex = lines.findIndex(
      (l) => l.trim() === 'projects:' || l.trim() === 'projects'
    );

    if (projectsIndex !== -1 && currentLineNum > projectsIndex && !trimmedLine.startsWith('-')) {
      const match = trimmedLine.match(/^([a-zA-Z0-9_\u4e00-\u9fa5\s]+)$/);
      if (match && match[1].length >= 1) {
        const query = match[1].trim().toLowerCase();
        const voiceTypingName = t('settings.voice_typing', {
          defaultValue: 'Voice Typing',
        });
        const allScopes = [
          { id: VOICE_TYPING_SCOPE_ID, name: voiceTypingName },
          ...projects.map((p) => ({ id: p.id, name: p.name })),
        ];
        const matches = allScopes.filter(
          (p) => p.name.toLowerCase().includes(query) || p.id.toLowerCase().includes(query)
        );
        if (matches.length > 0) {
          setSuggestions(matches);
          setSuggestionPos({ top: currentLineNum * 20, left: 60 });
          return;
        }
      }
    }

    setSuggestions([]);
    setSuggestionPos(null);
  };

  const handleSelectSuggestion = (project: { id: string; name: string }) => {
    if (!textareaRef.current) return;
    const lines = text.split('\n');
    lines[cursorLine - 1] = `  ${project.name} (id:${project.id}):`;
    if (lines[cursorLine] && !lines[cursorLine].trim().startsWith('-')) {
      lines.splice(cursorLine, 0, '    - ');
    } else if (!lines[cursorLine]) {
      lines.push('    - ');
    }
    const nextText = lines.join('\n');
    syncChanges(nextText);
    setSuggestions([]);
    setSuggestionPos(null);
  };

  const linesCount = useMemo(() => text.split('\n').length, [text]);

  const scopeOptions = useMemo(() => {
    return [
      {
        value: 'global',
        label: t('settings.dict_scope_global', { defaultValue: 'Global (All)' }),
      },
      {
        value: VOICE_TYPING_SCOPE_ID,
        label: t('settings.dict_scope_voice_typing', { defaultValue: 'Voice Typing' }),
      },
      ...projects.map((p) => ({
        value: p.id,
        label: p.name,
      })),
    ];
  }, [projects, t]);

  return (
    <SettingsSection
      title={t('settings.unified_dictionary_title', { defaultValue: 'Unified Dictionary' })}
      icon={<Book size={20} />}
      description={t('settings.unified_dictionary_desc', {
        defaultValue:
          'Manage global terms, text replacements, and project-specific vocabularies in one unified, human-readable format.',
      })}
    >
      {/* ── 1. Header Toolbar ── */}
      <div className="dict-toolbar">
        <div className="dict-mode-switcher" role="tablist" aria-label="Dictionary view mode">
          <button
            type="button"
            role="tab"
            aria-selected={mode === 'visual'}
            onClick={() => setMode('visual')}
            className="dict-mode-btn"
          >
            <Eye size={13} />
            {t('settings.dict_mode_visual', { defaultValue: 'Visual' })}
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={mode === 'code'}
            onClick={() => setMode('code')}
            className="dict-mode-btn"
          >
            <Code2 size={13} />
            {t('settings.dict_mode_code', { defaultValue: 'YAML Code' })}
          </button>
        </div>

        <div className="dict-toolbar-actions">
          {mode === 'visual' && (
            <button type="button" className="btn btn-primary btn-xs" onClick={openNewModal}>
              <Plus size={14} />
              {t('settings.dict_add_entry', { defaultValue: 'New Entry' })}
            </button>
          )}

          <button
            type="button"
            className="btn btn-secondary btn-xs"
            onClick={handleFormat}
            data-tooltip={t('settings.dict_format_tooltip', {
              defaultValue: 'Format indentation, canonicalize arrows, and sync project links',
            })}
            data-tooltip-pos="top"
            aria-label={t('settings.dict_format', { defaultValue: 'Format' })}
          >
            {formatFeedback ? (
              <Check size={13} style={{ color: 'var(--color-success)' }} />
            ) : (
              <Wand2 size={13} />
            )}
            {formatFeedback
              ? t('common.formatted', { defaultValue: 'Formatted' })
              : t('settings.dict_format', { defaultValue: 'Format' })}
          </button>

          <button
            type="button"
            className="btn btn-icon btn-secondary-soft btn-xs"
            onClick={handleCopyAll}
            data-tooltip={
              copyFeedback
                ? t('common.copied', { defaultValue: 'Copied' })
                : t('settings.dict_copy_all_tooltip', {
                    defaultValue: 'Copy dictionary YAML to clipboard',
                  })
            }
            data-tooltip-pos="top"
            aria-label={t('settings.dict_copy_all_tooltip', {
              defaultValue: 'Copy dictionary YAML to clipboard',
            })}
          >
            {copyFeedback ? (
              <Check size={13} style={{ color: 'var(--color-success)' }} />
            ) : (
              <Copy size={13} />
            )}
          </button>
        </div>
      </div>

      {/* ── 2. Quick Add Bar ── */}
      <div className="dict-quick-add">
        <div className="dict-quick-add-scope">
          <span className="dict-quick-add-scope-label">
            {t('settings.dict_target_scope', { defaultValue: 'Target:' })}
          </span>
          <div style={{ width: '150px' }}>
            <Dropdown
              id="unified-dict-scope-select"
              value={quickAddTarget}
              onChange={(val) => setQuickAddTarget(val)}
              options={scopeOptions}
              aria-label={t('settings.dict_target_scope', { defaultValue: 'Target:' })}
            />
          </div>
        </div>

        <div className="dict-quick-add-input-wrap">
          <input
            id="unified-dict-quick-add"
            type="text"
            className="settings-input"
            value={quickAddInput}
            onChange={(e) => setQuickAddInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                handleQuickAdd();
              }
            }}
            placeholder={t('settings.unified_dict_input_placeholder', {
              defaultValue: 'Term (e.g. Sona) or From -> To',
            })}
            style={{ width: '100%', height: '36px', fontSize: '0.8125rem', paddingRight: '60px' }}
          />
          {quickAddInput && (
            <span className="dict-quick-add-badge">
              {quickAddInput.includes('->') || quickAddInput.includes('=>') ? 'Replace' : 'Term'}
            </span>
          )}
        </div>

        <button
          type="button"
          className="btn btn-primary btn-xs"
          onClick={handleQuickAdd}
          disabled={!quickAddInput.trim()}
        >
          <Plus size={14} />
          {t('common.add', { defaultValue: 'Add' })}
        </button>
      </div>

      {/* ── 3. Visual Mode ── */}
      {mode === 'visual' && (
        <div className="dict-visual">
          {/* Scope Filters & Search */}
          <div className="dict-filters">
            <div className="dict-scope-pills">
              {[
                {
                  id: 'all',
                  label: t('settings.dict_scope_filter_all', { defaultValue: 'All Scopes' }),
                  count: scopeCounts.all,
                },
                {
                  id: 'global',
                  label: t('settings.dict_scope_badge_global', { defaultValue: 'Global' }),
                  count: scopeCounts.global,
                },
                {
                  id: VOICE_TYPING_SCOPE_ID,
                  label: t('settings.dict_scope_badge_vt', { defaultValue: 'Voice Typing' }),
                  count: scopeCounts[VOICE_TYPING_SCOPE_ID] || 0,
                },
                ...projects.map((p) => ({
                  id: p.id,
                  label: p.name,
                  count: scopeCounts[p.id] || 0,
                })),
              ].map((scope) => (
                <button
                  key={scope.id}
                  type="button"
                  onClick={() => setScopeFilter(scope.id)}
                  className="dict-scope-pill"
                  aria-pressed={scopeFilter === scope.id}
                >
                  <span>{scope.label}</span>
                  <span className="dict-scope-pill-count">{scope.count}</span>
                </button>
              ))}
            </div>

            {/* Search Box */}
            <div className="dict-search-wrap">
              <Search size={14} className="dict-search-icon" />
              <input
                type="text"
                className="settings-input"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder={t('settings.dict_search_placeholder', {
                  defaultValue: 'Search hotwords or replacements...',
                })}
                style={{
                  width: '100%',
                  height: '30px',
                  paddingLeft: '30px',
                  paddingRight: searchQuery ? '26px' : '10px',
                  fontSize: '0.75rem',
                }}
              />
              {searchQuery && (
                <button
                  type="button"
                  className="dict-search-clear"
                  onClick={() => setSearchQuery('')}
                >
                  <X size={13} />
                </button>
              )}
            </div>
          </div>

          {/* Hotwords Card */}
          <div className="dict-card">
            <div className="dict-card-header">
              <div className="dict-card-title">
                <span className="dict-card-title-icon">
                  <Sparkles size={15} />
                </span>
                {t('settings.dict_hotwords_title', { defaultValue: 'Hotwords & Vocabulary' })}
                <span className="dict-card-count">{hotwordsList.length}</span>
              </div>
              <div className="dict-card-desc">
                {t('settings.dict_hotwords_desc', {
                  defaultValue:
                    'Bias speech recognition accuracy for specific names, jargon, or keywords.',
                })}
              </div>
            </div>

            <div className="dict-card-body">
              <div className="dict-chips">
                {hotwordsList.map((item) => (
                  <div key={item.id} className="dict-chip">
                    <span className="dict-chip-text">{item.text}</span>
                    {item.weight && (
                      <span className="dict-chip-weight">x{item.weight.toFixed(1)}</span>
                    )}
                    {scopeFilter === 'all' && (
                      <span className="dict-chip-scope">{item.scopeName}</span>
                    )}
                    <button
                      type="button"
                      className="dict-chip-action"
                      onClick={() => openEditModal(item)}
                      aria-label={`${t('settings.dict_edit', { defaultValue: 'Edit' })} ${item.text}`}
                    >
                      <Pencil size={12} />
                    </button>
                    <button
                      type="button"
                      className="dict-chip-action danger"
                      onClick={() => handleDeleteTerm(item)}
                      aria-label={`${t('settings.dict_delete', { defaultValue: 'Delete' })} ${item.text}`}
                    >
                      <X size={13} />
                    </button>
                  </div>
                ))}

                {/* Inline Quick Add Input */}
                <input
                  type="text"
                  className="dict-inline-input"
                  value={inlineHotwordInput}
                  onChange={(e) => setInlineHotwordInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      handleInlineAddHotword();
                    }
                  }}
                  placeholder={t('settings.dict_add_hotword_inline_placeholder', {
                    defaultValue: '+ Add hotword...',
                  })}
                />
              </div>
            </div>
          </div>

          {/* Replacements Card */}
          <div className="dict-card">
            <div className="dict-card-header">
              <div className="dict-card-title">
                <span className="dict-card-title-icon">
                  <ArrowRight size={15} />
                </span>
                {t('settings.dict_replacements_title', {
                  defaultValue: 'Text Replacements & Corrections',
                })}
                <span className="dict-card-count">{replacementsList.length}</span>
              </div>
              <div className="dict-card-desc">
                {t('settings.dict_replacements_desc', {
                  defaultValue:
                    'Automatically correct misrecognitions and expand snippets upon transcription.',
                })}
              </div>
            </div>

            <div className="dict-card-body">
              {replacementsList.length === 0 ? (
                <div className="dict-empty">
                  {t('settings.dict_empty_replacements', {
                    defaultValue: 'No replacement rules in current scope',
                  })}
                </div>
              ) : (
                <div className="dict-rules">
                  {replacementsList.map((item) => (
                    <div key={item.id} className="dict-rule">
                      <div className="dict-rule-pair">
                        <span className="dict-rule-from">{item.from}</span>
                        <ArrowRight size={13} className="dict-rule-arrow" />
                        <span className="dict-rule-to">{item.to}</span>
                      </div>
                      <div className="dict-rule-actions">
                        <span className="dict-rule-scope">{item.scopeName}</span>
                        <button
                          type="button"
                          className="dict-rule-action-btn"
                          onClick={() => openEditModal(item)}
                          aria-label={`${t('settings.dict_edit', { defaultValue: 'Edit' })} ${item.from}`}
                        >
                          <Pencil size={12} />
                        </button>
                        <button
                          type="button"
                          className="dict-rule-action-btn danger"
                          onClick={() => handleDeleteTerm(item)}
                          aria-label={`${t('settings.dict_delete', { defaultValue: 'Delete' })} ${item.from}`}
                        >
                          <Trash2 size={12} />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Live Interactive Replacement Tester */}
            <div className="dict-test">
              <div className="dict-test-label">
                {t('settings.dict_test_replacement_title', {
                  defaultValue: 'Test Replacement Rules',
                })}
              </div>
              <input
                type="text"
                className="settings-input"
                value={testInput}
                onChange={(e) => setTestInput(e.target.value)}
                placeholder={t('settings.dict_test_input_placeholder', {
                  defaultValue: 'Type or paste text to test replacement...',
                })}
                style={{ width: '100%', height: '30px', fontSize: '0.75rem' }}
              />
              {testInput && (
                <div className="dict-test-result">
                  <span className="dict-test-result-label">
                    {t('settings.dict_test_result', { defaultValue: 'Result:' })}
                  </span>
                  <strong>{testOutput}</strong>
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* ── 4. Code Mode ── */}
      {mode === 'code' && (
        <div>
          {/* Status Bar */}
          <div className="dict-code-status">
            <span className="dict-code-stat">
              {t('settings.dict_total_terms', { defaultValue: 'Total terms:' })} {totalTermsCount}
            </span>
            <span className="dict-code-stat">
              {t('settings.dict_global_terms', { defaultValue: 'Global:' })}{' '}
              {parsed.globalTerms.length}
            </span>
            <span className="dict-code-stat">
              {t('settings.dict_projects_count', { defaultValue: 'Projects:' })}{' '}
              {parsed.projects.length}
            </span>
          </div>

          {/* Editor with Line Numbers */}
          <div className="dict-code-editor-wrap">
            <div className="dict-code-editor-flex">
              {/* Gutter */}
              <div className="dict-code-gutter" aria-hidden="true">
                {Array.from({ length: linesCount }, (_, i) => (
                  <div key={i + 1} className="dict-code-gutter-line">
                    {i + 1}
                  </div>
                ))}
              </div>

              {/* Textarea */}
              <div className="dict-code-textarea-wrap">
                <textarea
                  id="unified-dict-yaml-editor"
                  ref={textareaRef}
                  aria-label={t('settings.unified_dict_editor_label', {
                    defaultValue: 'Unified Dictionary Editor',
                  })}
                  value={text}
                  onChange={handleTextareaChange}
                  onKeyDown={(e) => {
                    if (e.key === 'Tab') {
                      e.preventDefault();
                      const start = e.currentTarget.selectionStart;
                      const end = e.currentTarget.selectionEnd;
                      const newText = `${text.substring(0, start)}  ${text.substring(end)}`;
                      syncChanges(newText);
                      setTimeout(() => {
                        if (textareaRef.current) {
                          textareaRef.current.selectionStart = textareaRef.current.selectionEnd =
                            start + 2;
                        }
                      }, 0);
                    }
                  }}
                  spellCheck={false}
                  className="dict-code-textarea"
                />

                {/* Project Suggestions Autocomplete */}
                {suggestions.length > 0 && suggestionPos && (
                  <div
                    className="dict-autocomplete"
                    style={{
                      top: `${suggestionPos.top}px`,
                      left: `${suggestionPos.left}px`,
                    }}
                  >
                    <div className="dict-autocomplete-header">
                      {t('settings.dict_autocomplete_header', { defaultValue: 'Link Project' })}
                    </div>
                    {suggestions.map((p) => (
                      <button
                        key={p.id}
                        type="button"
                        className="dict-autocomplete-item"
                        onClick={() => handleSelectSuggestion(p)}
                      >
                        <strong>{p.name}</strong>{' '}
                        <span className="dict-autocomplete-item-id">(id:{p.id})</span>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Diagnostics Panel */}
          {diagnostics.length > 0 && (
            <div className="dict-diagnostics">
              <div className="dict-diagnostics-header">
                {errorCount > 0 ? (
                  <AlertCircle size={15} style={{ color: 'var(--color-error)' }} />
                ) : (
                  <AlertTriangle size={15} style={{ color: 'var(--color-warning)' }} />
                )}
                <span>
                  {t('settings.dict_diagnostics_title', { defaultValue: 'Syntax & Link Issues' })} (
                  {diagnostics.length})
                </span>
              </div>

              <div className="dict-diagnostics-list">
                {diagnostics.map((diag, i) => (
                  <div
                    key={`${diag.line}-${i}`}
                    className="dict-diagnostic-item"
                    onClick={() => jumpToLine(diag.line)}
                    title={t('settings.dict_click_to_jump', {
                      defaultValue: 'Click to jump to line',
                    })}
                  >
                    <span
                      className={`dict-diagnostic-line ${diag.severity === 'error' ? 'error' : 'warning'}`}
                    >
                      Line {diag.line}:
                    </span>
                    <span className="dict-diagnostic-msg">{diag.message}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Syntax Tutorial & Guide */}
          <button
            type="button"
            className="dict-tutorial-toggle"
            onClick={() => setIsTutorialOpen(!isTutorialOpen)}
          >
            <span className="dict-tutorial-title">
              <HelpCircle size={15} />
              {t('settings.dict_tutorial_title', { defaultValue: 'Syntax Guide & Examples' })}
            </span>
            <span className="dict-tutorial-hint">
              {isTutorialOpen
                ? t('settings.dict_tutorial_collapse', { defaultValue: 'Hide guide' })
                : t('settings.dict_tutorial_expand', { defaultValue: 'Show guide' })}
              <ChevronDown
                size={14}
                style={{
                  transform: isTutorialOpen ? 'rotate(180deg)' : 'none',
                  transition: 'transform 0.2s ease',
                }}
              />
            </span>
          </button>

          {isTutorialOpen && (
            <div className="dict-tutorial-grid">
              <div className="dict-tutorial-block">
                <div className="dict-tutorial-block-title">
                  {t('settings.dict_tutorial_terms_title', {
                    defaultValue: '1. Terms & Hotwords',
                  })}
                </div>
                <p>
                  {t('settings.dict_tutorial_terms_desc', {
                    defaultValue:
                      'Add custom vocabulary or names to bias speech recognition accuracy.',
                  })}
                </p>
                <pre>{'- Sona\n- DeepSeek\n- Sherpa-onnx :2.0'}</pre>
                <span className="dict-tutorial-hint-text">
                  {t('settings.dict_tutorial_weights_desc', {
                    defaultValue:
                      'Tip: ":2.0" weight suffix is supported by sherpa-onnx transducer models.',
                  })}
                </span>
              </div>

              <div className="dict-tutorial-block">
                <div className="dict-tutorial-block-title">
                  {t('settings.dict_tutorial_replacements_title', {
                    defaultValue: '2. Replacements & Corrections',
                  })}
                </div>
                <p>
                  {t('settings.dict_tutorial_replacements_desc', {
                    defaultValue:
                      'Use "->" or "=>" to automatically correct misrecognitions and expand snippets.',
                  })}
                </p>
                <pre>
                  {'- 苦伯内提斯 -> Kubernetes\n- 微信支付 => WeChat Pay\n- btw -> by the way'}
                </pre>
              </div>

              <div className="dict-tutorial-block">
                <div className="dict-tutorial-block-title">
                  {t('settings.dict_tutorial_projects_title', {
                    defaultValue: '3. Project-Specific Scoping',
                  })}
                </div>
                <p>
                  {t('settings.dict_tutorial_projects_desc', {
                    defaultValue:
                      'Define terms under "projects:" with "(id:project_id)" to only apply during project transcription.',
                  })}
                </p>
                <pre>
                  {
                    'projects:\n  Voice Typing (id:voice-typing):\n    - Dictation Hotword\n  Project Name (id:proj-1):\n    - Sprint Planning\n    - PRD -> Product Doc'
                  }
                </pre>
              </div>

              <div className="dict-tutorial-block">
                <div className="dict-tutorial-block-title">
                  {t('settings.dict_tutorial_tips_title', {
                    defaultValue: '4. Tips & Shortcuts',
                  })}
                </div>
                <ul>
                  <li>
                    {t('settings.dict_tutorial_tip_tab', {
                      defaultValue: 'Tab key inserts 2-space indentation.',
                    })}
                  </li>
                  <li>
                    {t('settings.dict_tutorial_tip_format', {
                      defaultValue:
                        'Click "Format" to re-align spacing and synchronize renamed projects.',
                    })}
                  </li>
                  <li>
                    {t('settings.dict_tutorial_tip_click_diag', {
                      defaultValue:
                        'Click any issue in the diagnostics panel to jump directly to the line.',
                    })}
                  </li>
                </ul>
              </div>
            </div>
          )}
        </div>
      )}

      {/* ── 5. Add / Edit Term Modal ── */}
      {isAddModalOpen && (
        <Modal
          isOpen={isAddModalOpen}
          onClose={() => {
            setIsAddModalOpen(false);
            setEditingItem(null);
          }}
          title={
            editingItem
              ? t('settings.dict_modal_edit_title', { defaultValue: 'Edit Vocabulary Entry' })
              : t('settings.dict_modal_new_title', { defaultValue: 'New Vocabulary Entry' })
          }
          size="md"
          footer={
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => {
                  setIsAddModalOpen(false);
                  setEditingItem(null);
                }}
              >
                {t('common.cancel', { defaultValue: 'Cancel' })}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => handleModalSubmit()}
                disabled={
                  formType === 'hotword' ? !formWord.trim() : !formFrom.trim() || !formTo.trim()
                }
              >
                {t('settings.dict_action_save', { defaultValue: 'Save Entry' })}
              </button>
            </div>
          }
        >
          <form onSubmit={handleModalSubmit} className="dict-modal-form">
            {/* Entry Type */}
            <div>
              <label className="dict-modal-label">
                {t('settings.dict_entry_type', { defaultValue: 'Entry Type' })}
              </label>
              <div className="dict-modal-type-grid">
                <button
                  type="button"
                  className="dict-modal-type-btn"
                  aria-pressed={formType === 'hotword'}
                  onClick={() => setFormType('hotword')}
                >
                  <Sparkles size={14} />
                  {t('settings.dict_entry_type_hotword', { defaultValue: 'Hotword / Term' })}
                </button>
                <button
                  type="button"
                  className="dict-modal-type-btn"
                  aria-pressed={formType === 'replacement'}
                  onClick={() => setFormType('replacement')}
                >
                  <ArrowRight size={14} />
                  {t('settings.dict_entry_type_replacement', {
                    defaultValue: 'Replacement Rule',
                  })}
                </button>
              </div>
            </div>

            {/* Target Scope */}
            <div>
              <label htmlFor="modal-term-scope" className="dict-modal-label">
                {t('settings.dict_target_scope', { defaultValue: 'Target:' })}
              </label>
              <Dropdown
                id="modal-term-scope"
                value={formScope}
                onChange={(val) => setFormScope(val)}
                options={scopeOptions}
                style={{ width: '100%' }}
              />
            </div>

            {/* Hotword Form Fields */}
            {formType === 'hotword' ? (
              <>
                <div>
                  <label htmlFor="modal-field-word" className="dict-modal-label">
                    {t('settings.dict_field_word', { defaultValue: 'Term / Hotword' })}
                  </label>
                  <input
                    id="modal-field-word"
                    type="text"
                    className="settings-input"
                    value={formWord}
                    onChange={(e) => setFormWord(e.target.value)}
                    placeholder={t('settings.dict_field_word_placeholder', {
                      defaultValue: 'e.g. Sona or DeepSeek',
                    })}
                    autoFocus
                    style={{ width: '100%', height: '36px' }}
                  />
                </div>

                <div>
                  <label htmlFor="modal-field-weight" className="dict-modal-label">
                    {t('settings.dict_field_weight', { defaultValue: 'Weight Boost (Optional)' })}
                  </label>
                  <input
                    id="modal-field-weight"
                    type="number"
                    step="0.1"
                    min="1.0"
                    max="5.0"
                    className="settings-input"
                    value={formWeight}
                    onChange={(e) => setFormWeight(e.target.value)}
                    placeholder="e.g. 2.0"
                    style={{ width: '100%', height: '36px' }}
                  />
                </div>
              </>
            ) : (
              /* Replacement Form Fields */
              <>
                <div>
                  <label htmlFor="modal-field-from" className="dict-modal-label">
                    {t('settings.dict_field_from', { defaultValue: 'Original Text (From)' })}
                  </label>
                  <input
                    id="modal-field-from"
                    type="text"
                    className="settings-input"
                    value={formFrom}
                    onChange={(e) => setFormFrom(e.target.value)}
                    placeholder={t('settings.dict_field_from_placeholder', {
                      defaultValue: 'Misrecognized word (e.g. 苦伯内提斯)',
                    })}
                    autoFocus
                    style={{ width: '100%', height: '36px' }}
                  />
                </div>

                <div>
                  <label htmlFor="modal-field-to" className="dict-modal-label">
                    {t('settings.dict_field_to', { defaultValue: 'Target Replacement (To)' })}
                  </label>
                  <input
                    id="modal-field-to"
                    type="text"
                    className="settings-input"
                    value={formTo}
                    onChange={(e) => setFormTo(e.target.value)}
                    placeholder={t('settings.dict_field_to_placeholder', {
                      defaultValue: 'Corrected word (e.g. Kubernetes)',
                    })}
                    style={{ width: '100%', height: '36px' }}
                  />
                </div>
              </>
            )}
          </form>
        </Modal>
      )}
    </SettingsSection>
  );
}
