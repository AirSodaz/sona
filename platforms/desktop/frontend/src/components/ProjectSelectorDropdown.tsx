import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
function InboxIcon(props?: React.SVGProps<SVGSVGElement>): React.JSX.Element {
  return (
    <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11Z" />
      <polyline points="22 12 16 12 14 15 10 15 8 12 2 12" />
    </svg>
  );
}

function ChevronDownIcon(props?: React.SVGProps<SVGSVGElement>): React.JSX.Element {
  return (
    <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <polyline points="6 9 12 15 18 9" />
    </svg>
  );
}

function CheckIcon(props?: React.SVGProps<SVGSVGElement>): React.JSX.Element {
  return (
    <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <polyline points="20 6 9 17 4 12" />
    </svg>
  );
}

function SearchIcon(props?: React.SVGProps<SVGSVGElement>): React.JSX.Element {
  return (
    <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <circle cx="11" cy="11" r="8" />
      <line x1="21" y1="21" x2="16.65" y2="16.65" />
    </svg>
  );
}

function PlusIcon(props?: React.SVGProps<SVGSVGElement>): React.JSX.Element {
  return (
    <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <line x1="12" y1="5" x2="12" y2="19" />
      <line x1="5" y1="12" x2="19" y2="12" />
    </svg>
  );
}

function ZapIcon(props?: React.SVGProps<SVGSVGElement>): React.JSX.Element {
  return (
    <svg aria-hidden="true" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
    </svg>
  );
}
import { useProjectStore } from '../stores/projectStore';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { ProjectCreateModal } from './projects/ProjectCreateModal';
import { ProjectVisual } from './projects/ProjectVisual';
type Props = { onOpenProjects: () => void };

export function ProjectSelectorDropdown({ onOpenProjects }: Props): React.JSX.Element {
  const { t } = useTranslation();
  const projectsValue = useProjectStore((state) => state.projects);
  const projects = useMemo(() => projectsValue ?? [], [projectsValue]);
  const activeProjectId = useProjectStore((state) => state.activeProjectId) ?? null;
  const setActiveProjectId = useProjectStore((state) => state.setActiveProjectId) ?? (async () => undefined);
  const createProject = useProjectStore((state) => state.createProject) ?? (async () => null);
  const historyItems = useHistoryStore((state) => state.items);

  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const [createOpen, setCreateOpen] = useState(false);
  const [createName, setCreateName] = useState('');
  const [createDescription, setCreateDescription] = useState('');
  const [createColor, setCreateColor] = useState('#6366F1');
  const [createIcon, setCreateIcon] = useState('');
  const ref = useRef<HTMLDivElement>(null);

  const countInbox = useMemo(() => historyItems.filter((i) => !i.deletedAt && !i.projectId).length, [historyItems]);
  const projectCounts = useMemo(() => {
    const map = new Map<string, number>();
    for (const item of historyItems) {
      if (!item.deletedAt && item.projectId) {
        map.set(item.projectId, (map.get(item.projectId) || 0) + 1);
      }
    }
    return map;
  }, [historyItems]);

  const locked = isRecording && !!sourceHistoryId;
  const active = projects.find((project) => project.id === activeProjectId) ?? null;
  const filtered = useMemo(() => projects.filter((project) => project.name.toLowerCase().includes(query.trim().toLowerCase())), [projects, query]);
  const options = [null, ...filtered.map((project) => project.id)];

  useEffect(() => {
    const close = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, []);

  const choose = (id: string | null) => {
    void setActiveProjectId(id);
    setOpen(false);
    setQuery('');
  };

  const title = active?.name ?? t('projects.inbox', { defaultValue: '收件箱' });

  return (
    <>
      <div className="project-selector" ref={ref}>
        <button
          type="button"
          className="project-selector-trigger"
          disabled={locked}
          onClick={() => setOpen((value) => !value)}
          aria-haspopup="listbox"
          aria-expanded={open}
          title={locked ? t('projects.locked_during_recording', { defaultValue: '录音中无法切换项目' }) : undefined}
        >
          {active ? (
            <ProjectVisual icon={active.icon} color={active.color} size="xs" showBackground />
          ) : (
            <InboxIcon width={14} height={14} />
          )}
          <span>{title}</span>
          {active?.pipeline?.enabled && <ZapIcon aria-label={t('projects.pipeline_enabled', { defaultValue: '流水线已启用' })} />}
          <ChevronDownIcon width={14} height={14} />
        </button>

        {open && (
          <div
            className="project-selector-popover"
            role="listbox"
            tabIndex={0}
            onKeyDown={(event) => {
              if (event.key === 'Escape') setOpen(false);
              if (event.key === 'ArrowDown') {
                event.preventDefault();
                setIndex((value) => Math.min(value + 1, options.length - 1));
              }
              if (event.key === 'ArrowUp') {
                event.preventDefault();
                setIndex((value) => Math.max(value - 1, 0));
              }
              if (event.key === 'Enter') choose(options[index]);
            }}
          >
            <label className="project-selector-search">
              <SearchIcon />
              <input
                autoFocus
                value={query}
                onChange={(event) => {
                  setQuery(event.target.value);
                  setIndex(0);
                }}
                placeholder={t('projects.search_projects', { defaultValue: '搜索项目...' })}
              />
            </label>

            <button
              type="button"
              className={`project-selector-option ${activeProjectId === null ? 'active' : ''}`}
              onClick={() => choose(null)}
            >
              <InboxIcon width={14} height={14} />
              <span>{t('projects.inbox', { defaultValue: '收件箱' })}</span>
              {activeProjectId === null && <CheckIcon width={14} height={14} />}
              <span className="project-selector-count">{countInbox}</span>
            </button>

            {filtered.map((project) => (
              <button
                type="button"
                role="option"
                key={project.id}
                className={`project-selector-option ${activeProjectId === project.id ? 'active' : ''}`}
                onClick={() => choose(project.id)}
              >
                <ProjectVisual icon={project.icon} color={project.color} size="xs" showBackground />
                <span>{project.name}</span>
                {project.pipeline?.enabled && <ZapIcon />}
                {activeProjectId === project.id && <CheckIcon width={14} height={14} />}
                <span className="project-selector-count">{projectCounts.get(project.id) || 0}</span>
              </button>
            ))}

            <div className="project-selector-footer">
              <button
                type="button"
                onClick={() => {
                  setCreateName('');
                  setCreateDescription('');
                  setCreateOpen(true);
                }}
              >
                <PlusIcon />
                {t('projects.new_project_button', { defaultValue: '新建项目' })}
              </button>
              <button type="button" onClick={onOpenProjects}>
                {t('projects.open_projects_center', { defaultValue: '打开项目中心' })}
              </button>
            </div>
          </div>
        )}
      </div>

      <ProjectCreateModal
        isOpen={createOpen}
        name={createName}
        description={createDescription}
        color={createColor}
        icon={createIcon}
        onNameChange={setCreateName}
        onDescriptionChange={setCreateDescription}
        onColorChange={setCreateColor}
        onIconChange={setCreateIcon}
        onClose={() => setCreateOpen(false)}
        onCreate={async () => {
          const project = await createProject({
            name: createName,
            description: createDescription,
            color: createColor,
            icon: createIcon,
          });
          if (project) await setActiveProjectId(project.id);
          setCreateIcon('');
          setCreateOpen(false);
        }}
      />
    </>
  );
}
