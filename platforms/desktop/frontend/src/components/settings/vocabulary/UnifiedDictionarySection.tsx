import {
  AlertCircle,
  AlertTriangle,
  Book,
  Check,
  ChevronDown,
  Copy,
  HelpCircle,
  Plus,
  Wand2,
} from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useProjectStore } from '../../../stores/projectStore';
import type { HotwordRuleSet, TextReplacementRuleSet } from '../../../types/config';
import {
  formatYamlDictionary,
  parseYamlDictionary,
  serializeToYamlDictionary,
  validateYamlDictionary,
} from '../../../utils/yamlDictionaryParser';
import { Dropdown } from '../../Dropdown';
import { SettingsSection } from '../SettingsLayout';

interface UnifiedDictionarySectionProps {
  content?: string;
  onUpdateContent: (content: string) => void;
  hotwordSets?: HotwordRuleSet[];
  textReplacementSets?: TextReplacementRuleSet[];
  onUpdateHotwordSets?: (sets: HotwordRuleSet[]) => void;
  onUpdateTextReplacementSets?: (sets: TextReplacementRuleSet[]) => void;
}

export function UnifiedDictionarySection({
  content = '',
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
  const [quickAddInput, setQuickAddInput] = useState('');
  const [quickAddTarget, setQuickAddTarget] = useState<string>('global'); // 'global' | project.id
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

  // Parsed stats
  const parsed = useMemo(() => {
    return parseYamlDictionary(text, projects);
  }, [text, projects]);

  const totalTermsCount =
    parsed.globalTerms.length + parsed.projects.reduce((sum, p) => sum + p.terms.length, 0);

  // Format action
  const handleFormat = () => {
    const formatted = formatYamlDictionary(text, projects);
    syncChanges(formatted);
    setFormatFeedback(true);
    setTimeout(() => setFormatFeedback(false), 1500);
  };

  // Copy all action
  const handleCopyAll = () => {
    void navigator.clipboard.writeText(text).then(() => {
      setCopyFeedback(true);
      setTimeout(() => setCopyFeedback(false), 1500);
    });
  };

  // Quick Add action
  const handleQuickAdd = () => {
    const raw = quickAddInput.trim();
    if (!raw) return;

    let entry = `- ${raw}`;
    if (raw.includes('=>')) {
      entry = `- ${raw.replace('=>', '->')}`;
    } else if (!raw.startsWith('-') && !raw.includes('->')) {
      entry = `- ${raw}`;
    }

    let updated = text;
    if (quickAddTarget === 'global') {
      const lines = updated.split('\n');
      const projectsIdx = lines.findIndex(
        (l) => l.trim() === 'projects:' || l.trim() === 'projects'
      );
      if (projectsIdx >= 0) {
        lines.splice(projectsIdx, 0, entry);
        updated = lines.join('\n');
      } else {
        updated = `${updated.trimEnd()}\n${entry}\n`;
      }
    } else {
      const targetProj = projects.find((p) => p.id === quickAddTarget);
      if (targetProj) {
        const lines = updated.split('\n');
        let projectsIdx = lines.findIndex(
          (l) => l.trim() === 'projects:' || l.trim() === 'projects'
        );
        if (projectsIdx === -1) {
          lines.push('', 'projects:');
          projectsIdx = lines.length - 1;
        }

        const projHeaderIdx = lines.findIndex(
          (l, i) => i > projectsIdx && l.includes(`id:${targetProj.id}`)
        );

        if (projHeaderIdx >= 0) {
          lines.splice(projHeaderIdx + 1, 0, `    ${entry}`);
        } else {
          lines.push(`  ${targetProj.name} (id:${targetProj.id}):`, `    ${entry}`);
        }
        updated = lines.join('\n');
      }
    }

    syncChanges(updated);
    setQuickAddInput('');
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

    // Check if user is typing in projects section and wants to link a project
    const lines = val.split('\n');
    const projectsIndex = lines.findIndex(
      (l) => l.trim() === 'projects:' || l.trim() === 'projects'
    );

    if (projectsIndex !== -1 && currentLineNum > projectsIndex && !trimmedLine.startsWith('-')) {
      const match = trimmedLine.match(/^([a-zA-Z0-9_\u4e00-\u9fa5\s]+)$/);
      if (match && match[1].length >= 1) {
        const query = match[1].trim().toLowerCase();
        const matches = projects.filter((p) => p.name.toLowerCase().includes(query));
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

  // Line numbers
  const linesCount = useMemo(() => text.split('\n').length, [text]);

  const scopeOptions = useMemo(() => {
    return [
      {
        value: 'global',
        label: t('settings.dict_scope_global', { defaultValue: 'Global (All)' }),
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
      {/* 1. Quick Add Bar */}
      <div
        style={{
          display: 'flex',
          gap: '10px',
          padding: '16px 20px',
          background: 'var(--color-bg-primary)',
          alignItems: 'center',
          borderBottom: '1px solid var(--color-border-subtle)',
          flexWrap: 'wrap',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span
            style={{ fontSize: '0.8125rem', color: 'var(--color-text-secondary)', fontWeight: 500 }}
          >
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

        <div style={{ flex: 1, minWidth: '200px', position: 'relative' }}>
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
            <span
              style={{
                position: 'absolute',
                right: '10px',
                top: '50%',
                transform: 'translateY(-50%)',
                fontSize: '0.75rem',
                color: 'var(--color-text-muted)',
                pointerEvents: 'none',
              }}
            >
              {quickAddInput.includes('->') || quickAddInput.includes('=>') ? 'Replace' : 'Term'}
            </span>
          )}
        </div>

        <button
          type="button"
          className="btn btn-primary"
          onClick={handleQuickAdd}
          disabled={!quickAddInput.trim()}
          style={{
            height: '36px',
            display: 'inline-flex',
            alignItems: 'center',
            gap: '6px',
            padding: '0 16px',
          }}
        >
          <Plus size={15} />
          {t('common.add', { defaultValue: 'Add' })}
        </button>
      </div>

      {/* 2. Toolbar & Status */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          flexWrap: 'wrap',
          gap: '12px',
          padding: '10px 20px',
          background: 'var(--color-bg-secondary)',
          borderBottom: '1px solid var(--color-border-subtle)',
        }}
      >
        {/* Stats */}
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          <span
            style={{
              fontSize: '0.75rem',
              padding: '2px 8px',
              borderRadius: '9999px',
              background: 'var(--color-bg-primary)',
              border: '1px solid var(--color-border-subtle)',
              fontWeight: 500,
            }}
          >
            {t('settings.dict_total_terms', { defaultValue: 'Total terms:' })} {totalTermsCount}
          </span>
          <span
            style={{
              fontSize: '0.75rem',
              padding: '2px 8px',
              borderRadius: '9999px',
              background: 'var(--color-bg-primary)',
              border: '1px solid var(--color-border-subtle)',
              color: 'var(--color-text-secondary)',
            }}
          >
            {t('settings.dict_global_terms', { defaultValue: 'Global:' })}{' '}
            {parsed.globalTerms.length}
          </span>
          <span
            style={{
              fontSize: '0.75rem',
              padding: '2px 8px',
              borderRadius: '9999px',
              background: 'var(--color-bg-primary)',
              border: '1px solid var(--color-border-subtle)',
              color: 'var(--color-text-secondary)',
            }}
          >
            {t('settings.dict_projects_count', { defaultValue: 'Projects:' })}{' '}
            {parsed.projects.length}
          </span>
        </div>

        {/* Actions */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <button
            type="button"
            className="btn btn-secondary btn-xs"
            onClick={handleFormat}
            style={{ height: '28px', display: 'inline-flex', alignItems: 'center', gap: '4px' }}
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
            style={{
              width: '28px',
              height: '28px',
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              padding: 0,
            }}
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

      {/* 3. Editor with Line Numbers */}
      <div style={{ position: 'relative', background: 'var(--color-bg-primary)' }}>
        <div
          style={{
            display: 'flex',
            minHeight: '300px',
            maxHeight: '480px',
            overflowY: 'auto',
            fontFamily: 'Consolas, Monaco, "Courier New", monospace',
            fontSize: '13px',
            lineHeight: '20px',
          }}
        >
          {/* Gutter (Line Numbers) */}
          <div
            style={{
              width: '44px',
              padding: '12px 6px',
              textAlign: 'right',
              userSelect: 'none',
              background: 'var(--color-bg-secondary)',
              color: 'var(--color-text-muted)',
              borderRight: '1px solid var(--color-border-subtle)',
              opacity: 0.7,
              flexShrink: 0,
            }}
          >
            {Array.from({ length: linesCount }, (_, i) => (
              <div key={i + 1} style={{ height: '20px' }}>
                {i + 1}
              </div>
            ))}
          </div>

          {/* Textarea */}
          <div style={{ flex: 1, position: 'relative' }}>
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
              style={{
                width: '100%',
                minHeight: '300px',
                height: '100%',
                padding: '12px 16px',
                border: 'none',
                outline: 'none',
                background: 'transparent',
                color: 'var(--color-text-primary)',
                fontFamily: 'inherit',
                fontSize: 'inherit',
                lineHeight: 'inherit',
                resize: 'none',
                whiteSpace: 'pre',
              }}
            />

            {/* Project Suggestions Autocomplete */}
            {suggestions.length > 0 && suggestionPos && (
              <div
                style={{
                  position: 'absolute',
                  top: `${suggestionPos.top}px`,
                  left: `${suggestionPos.left}px`,
                  zIndex: 20,
                  background: 'var(--color-bg-primary)',
                  border: '1px solid var(--color-border)',
                  borderRadius: '6px',
                  boxShadow: '0 4px 12px rgba(0, 0, 0, 0.12)',
                  minWidth: '220px',
                  overflow: 'hidden',
                }}
              >
                <div
                  style={{
                    padding: '4px 8px',
                    fontSize: '0.6875rem',
                    color: 'var(--color-text-muted)',
                    background: 'var(--color-bg-secondary)',
                    fontWeight: 600,
                  }}
                >
                  {t('settings.dict_autocomplete_header', { defaultValue: 'Link Project' })}
                </div>
                {suggestions.map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    onClick={() => handleSelectSuggestion(p)}
                    style={{
                      display: 'block',
                      width: '100%',
                      textAlign: 'left',
                      padding: '6px 10px',
                      fontSize: '0.8125rem',
                      background: 'none',
                      border: 'none',
                      cursor: 'pointer',
                      color: 'var(--color-text-primary)',
                    }}
                    onMouseEnter={(e) =>
                      (e.currentTarget.style.background = 'var(--color-bg-secondary)')
                    }
                    onMouseLeave={(e) => (e.currentTarget.style.background = 'none')}
                  >
                    <strong>{p.name}</strong>{' '}
                    <span style={{ opacity: 0.6, fontSize: '0.75rem' }}>(id:{p.id})</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* 4. Diagnostics & Error Positioning Panel */}
      {diagnostics.length > 0 && (
        <div
          style={{
            borderTop: '1px solid var(--color-border-subtle)',
            background: 'var(--color-bg-secondary)',
            padding: '12px 20px',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
            {errorCount > 0 ? (
              <AlertCircle size={15} style={{ color: 'var(--color-danger, #ef4444)' }} />
            ) : (
              <AlertTriangle size={15} style={{ color: 'var(--color-warning, #f59e0b)' }} />
            )}
            <span
              style={{ fontSize: '0.8125rem', fontWeight: 600, color: 'var(--color-text-primary)' }}
            >
              {t('settings.dict_diagnostics_title', { defaultValue: 'Syntax & Link Issues' })} (
              {diagnostics.length})
            </span>
          </div>

          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: '4px',
              maxHeight: '120px',
              overflowY: 'auto',
            }}
          >
            {diagnostics.map((diag, i) => (
              <div
                key={`${diag.line}-${i}`}
                onClick={() => jumpToLine(diag.line)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                  fontSize: '0.75rem',
                  padding: '3px 8px',
                  borderRadius: '4px',
                  cursor: 'pointer',
                  background: 'var(--color-bg-primary)',
                  transition: 'background 0.15s ease',
                }}
                title={t('settings.dict_click_to_jump', { defaultValue: 'Click to jump to line' })}
              >
                <span
                  style={{
                    fontFamily: 'monospace',
                    fontWeight: 600,
                    color:
                      diag.severity === 'error'
                        ? 'var(--color-danger, #ef4444)'
                        : 'var(--color-warning, #f59e0b)',
                  }}
                >
                  Line {diag.line}:
                </span>
                <span style={{ color: 'var(--color-text-secondary)', flex: 1 }}>
                  {diag.message}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 5. Syntax Tutorial & Guide */}
      <div
        style={{
          borderTop: '1px solid var(--color-border-subtle)',
          background: 'var(--color-bg-primary)',
          padding: '14px 20px',
        }}
      >
        <button
          type="button"
          onClick={() => setIsTutorialOpen(!isTutorialOpen)}
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            width: '100%',
            background: 'none',
            border: 'none',
            padding: 0,
            cursor: 'pointer',
            textAlign: 'left',
          }}
        >
          <span
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: '6px',
              fontSize: '0.8125rem',
              fontWeight: 600,
              color: 'var(--color-text-primary)',
            }}
          >
            <HelpCircle size={15} style={{ color: 'var(--color-accent-primary, #6366f1)' }} />
            {t('settings.dict_tutorial_title', { defaultValue: 'Syntax Guide & Examples' })}
          </span>
          <span
            style={{
              fontSize: '0.75rem',
              color: 'var(--color-text-secondary)',
              display: 'inline-flex',
              alignItems: 'center',
              gap: '4px',
            }}
          >
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
          <div
            style={{
              marginTop: '12px',
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
              gap: '12px',
              fontSize: '0.75rem',
              lineHeight: 1.5,
              color: 'var(--color-text-secondary)',
            }}
          >
            {/* 1. Terms & Weights */}
            <div
              style={{
                padding: '10px 12px',
                background: 'var(--color-bg-secondary)',
                borderRadius: '6px',
                border: '1px solid var(--color-border-subtle)',
              }}
            >
              <div
                style={{ fontWeight: 600, color: 'var(--color-text-primary)', marginBottom: '4px' }}
              >
                {t('settings.dict_tutorial_terms_title', { defaultValue: '1. Terms & Hotwords' })}
              </div>
              <p style={{ margin: '0 0 6px 0' }}>
                {t('settings.dict_tutorial_terms_desc', {
                  defaultValue:
                    'Add custom vocabulary or names to bias speech recognition accuracy.',
                })}
              </p>
              <pre
                style={{
                  margin: 0,
                  padding: '6px 8px',
                  background: 'var(--color-bg-primary)',
                  borderRadius: '4px',
                  fontFamily: 'monospace',
                  fontSize: '0.75rem',
                }}
              >
                {'- Sona\n- DeepSeek\n- Sherpa-onnx :2.0'}
              </pre>
              <span
                style={{
                  fontSize: '0.6875rem',
                  color: 'var(--color-text-muted)',
                  display: 'block',
                  marginTop: '4px',
                }}
              >
                {t('settings.dict_tutorial_weights_desc', {
                  defaultValue:
                    'Tip: ":2.0" weight suffix is supported by sherpa-onnx transducer models.',
                })}
              </span>
            </div>

            {/* 2. Corrections & Replacements */}
            <div
              style={{
                padding: '10px 12px',
                background: 'var(--color-bg-secondary)',
                borderRadius: '6px',
                border: '1px solid var(--color-border-subtle)',
              }}
            >
              <div
                style={{ fontWeight: 600, color: 'var(--color-text-primary)', marginBottom: '4px' }}
              >
                {t('settings.dict_tutorial_replacements_title', {
                  defaultValue: '2. Replacements & Corrections',
                })}
              </div>
              <p style={{ margin: '0 0 6px 0' }}>
                {t('settings.dict_tutorial_replacements_desc', {
                  defaultValue:
                    'Use "->" or "=>" to automatically correct misrecognitions and expand snippets.',
                })}
              </p>
              <pre
                style={{
                  margin: 0,
                  padding: '6px 8px',
                  background: 'var(--color-bg-primary)',
                  borderRadius: '4px',
                  fontFamily: 'monospace',
                  fontSize: '0.75rem',
                }}
              >
                {'- 苦伯内提斯 -> Kubernetes\n- 微信支付 => WeChat Pay\n- btw -> by the way'}
              </pre>
            </div>

            {/* 3. Project Scoping */}
            <div
              style={{
                padding: '10px 12px',
                background: 'var(--color-bg-secondary)',
                borderRadius: '6px',
                border: '1px solid var(--color-border-subtle)',
              }}
            >
              <div
                style={{ fontWeight: 600, color: 'var(--color-text-primary)', marginBottom: '4px' }}
              >
                {t('settings.dict_tutorial_projects_title', {
                  defaultValue: '3. Project-Specific Scoping',
                })}
              </div>
              <p style={{ margin: '0 0 6px 0' }}>
                {t('settings.dict_tutorial_projects_desc', {
                  defaultValue:
                    'Define terms under "projects:" with "(id:project_id)" to only apply during project transcription.',
                })}
              </p>
              <pre
                style={{
                  margin: 0,
                  padding: '6px 8px',
                  background: 'var(--color-bg-primary)',
                  borderRadius: '4px',
                  fontFamily: 'monospace',
                  fontSize: '0.75rem',
                }}
              >
                {
                  'projects:\n  Project Name (id:proj-1):\n    - Sprint Planning\n    - PRD -> Product Doc'
                }
              </pre>
            </div>

            {/* 4. Tips & Shortcuts */}
            <div
              style={{
                padding: '10px 12px',
                background: 'var(--color-bg-secondary)',
                borderRadius: '6px',
                border: '1px solid var(--color-border-subtle)',
              }}
            >
              <div
                style={{ fontWeight: 600, color: 'var(--color-text-primary)', marginBottom: '4px' }}
              >
                {t('settings.dict_tutorial_tips_title', { defaultValue: '4. Tips & Shortcuts' })}
              </div>
              <ul
                style={{
                  margin: 0,
                  paddingLeft: '16px',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '3px',
                }}
              >
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
    </SettingsSection>
  );
}
