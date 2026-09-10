import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Check, ChevronDown, Inbox, Plus, Search, Zap } from 'lucide-react';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { ProjectCreateModal } from './projects/ProjectCreateModal';

type Props = { onOpenProjects: () => void };

export function ProjectSelectorDropdown({ onOpenProjects }: Props): React.JSX.Element {
  const projectsValue = useProjectStore((state) => state.projects);
  const projects = useMemo(() => projectsValue ?? [], [projectsValue]);
  const activeProjectId = useProjectStore((state) => state.activeProjectId) ?? null;
  const setActiveProjectId = useProjectStore((state) => state.setActiveProjectId) ?? (async () => undefined);
  const createProject = useProjectStore((state) => state.createProject) ?? (async () => null);
  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const [createOpen, setCreateOpen] = useState(false);
  const [createName, setCreateName] = useState('');
  const [createDescription, setCreateDescription] = useState('');
  const [createColor, setCreateColor] = useState('#6366F1');
  const ref = useRef<HTMLDivElement>(null);
  const locked = isRecording && !!sourceHistoryId;
  const active = projects.find((project) => project.id === activeProjectId) ?? null;
  const filtered = useMemo(() => projects.filter((project) => project.name.toLowerCase().includes(query.trim().toLowerCase())), [projects, query]);
  const options = [null, ...filtered.map((project) => project.id)];

  useEffect(() => {
    const close = (event: MouseEvent) => { if (!ref.current?.contains(event.target as Node)) setOpen(false); };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, []);

  const choose = (id: string | null) => { void setActiveProjectId(id); setOpen(false); setQuery(''); };
  const title = active?.name ?? '收件箱';

  return <>
    <div className="project-selector" ref={ref}>
      <button type="button" className="project-selector-trigger" disabled={locked} onClick={() => setOpen((value) => !value)} aria-haspopup="listbox" aria-expanded={open} title={locked ? '录音中无法切换项目' : undefined}>
        {active ? <span className="project-color-dot" style={{ '--project-color': active.color || '#6366F1' } as React.CSSProperties} /> : <Inbox size={14} />}
        <span>{title}</span>{active?.pipeline?.enabled && <Zap size={12} aria-label="流水线已启用" />}<ChevronDown size={14} />
      </button>
      {open && <div className="project-selector-popover" role="listbox" tabIndex={0} onKeyDown={(event) => {
        if (event.key === 'Escape') setOpen(false);
        if (event.key === 'ArrowDown') { event.preventDefault(); setIndex((value) => Math.min(value + 1, options.length - 1)); }
        if (event.key === 'ArrowUp') { event.preventDefault(); setIndex((value) => Math.max(value - 1, 0)); }
        if (event.key === 'Enter') choose(options[index]);
      }}>
        <label className="project-selector-search"><Search size={14} /><input autoFocus value={query} onChange={(event) => { setQuery(event.target.value); setIndex(0); }} placeholder="搜索项目..." /></label>
        <button type="button" className={`project-selector-option ${activeProjectId === null ? 'active' : ''}`} onClick={() => choose(null)}><Inbox size={14} /><span>收件箱</span>{activeProjectId === null && <Check size={14} />}</button>
        {filtered.map((project) => <button type="button" role="option" key={project.id} className={`project-selector-option ${activeProjectId === project.id ? 'active' : ''}`} onClick={() => choose(project.id)}><span className="project-color-dot" style={{ '--project-color': project.color || '#6366F1' } as React.CSSProperties} /><span>{project.name}</span>{project.pipeline?.enabled && <Zap size={12} />}{activeProjectId === project.id && <Check size={14} />}</button>)}
        <div className="project-selector-footer"><button type="button" onClick={() => { setCreateName(''); setCreateDescription(''); setCreateOpen(true); }}><Plus size={14} />新建项目</button><button type="button" onClick={onOpenProjects}>打开项目中心</button></div>
      </div>}
    </div>
    <ProjectCreateModal isOpen={createOpen} name={createName} description={createDescription} color={createColor} onNameChange={setCreateName} onDescriptionChange={setCreateDescription} onColorChange={setCreateColor} onClose={() => setCreateOpen(false)} onCreate={async () => { const project = await createProject({ name: createName, description: createDescription, color: createColor }); if (project) await setActiveProjectId(project.id); setCreateOpen(false); }} />
  </>;
}
