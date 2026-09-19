import { create } from 'zustand';
import { historyService } from '../services/historyService';
import { projectService } from '../services/projectService';
import type { ProjectPipelineConfig, ProjectRecord, ProjectUpdateInput } from '../types/project';
import { normalizeSpeakerProfiles } from '../types/speakerNormalization';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';
import { useConfigStore } from './configStore';

interface CreateProjectInput {
  name: string;
  description?: string;
  icon?: string;
  color?: string;
}

interface ProjectState {
  projects: ProjectRecord[];
  activeProjectId: string | null;
  isLoading: boolean;
  error: string | null;
  loadProjects: () => Promise<void>;
  createProject: (input: CreateProjectInput) => Promise<ProjectRecord | null>;
  updateProject: (id: string, updates: ProjectUpdateInput) => Promise<void>;
  updateProjectPipeline: (id: string, pipeline: ProjectPipelineConfig) => Promise<void>;
  deleteProject: (id: string, cascadeAction?: 'moveToInbox' | 'deleteItems') => Promise<void>;
  moveItemsToProject: (historyIds: string[], projectId: string | null) => Promise<void>;
  setActiveProjectId: (projectId: string | null) => Promise<void>;
  assignHistoryItems: (historyIds: string[], projectId: string | null) => Promise<void>;
  reorderProjects: (projectIds: string[]) => Promise<void>;
  getActiveProject: () => ProjectRecord | null;
  getProjectById: (projectId: string | null | undefined) => ProjectRecord | null;
}

export const useProjectStore = create<ProjectState>((set, get) => ({
  projects: [],
  activeProjectId: null,
  isLoading: false,
  error: null,

  loadProjects: async () => {
    set({ isLoading: true, error: null });
    try {
      const [projects, activeProjectId] = await Promise.all([
        projectService.getAll(),
        projectService.getActiveProjectId(),
      ]);

      const isValidActiveProject =
        !activeProjectId || projects.some((item) => item.id === activeProjectId);
      const normalizedActiveProjectId = isValidActiveProject ? activeProjectId : null;

      if (activeProjectId && !isValidActiveProject) {
        await projectService.setActiveProjectId(null);
      }

      set({
        projects,
        activeProjectId: normalizedActiveProjectId,
        isLoading: false,
      });
    } catch (error) {
      set({
        error: extractErrorMessage(error) || 'Failed to load projects',
        isLoading: false,
      });
    }
  },

  createProject: async (input) => {
    const project = await projectService.create({
      name: input.name,
      description: input.description || '',
      icon: input.icon || '',
      color: input.color || '#64748b',
    });

    set((state) => ({
      projects: [project, ...state.projects.filter((item) => item.id !== project.id)],
    }));
    return project;
  },

  updateProject: async (id, updates) => {
    const updated = await projectService.update(id, updates);
    if (!updated) {
      return;
    }

    set((state) => ({
      projects: state.projects.map((item) => (item.id === id ? updated : item)),
    }));
  },

  updateProjectPipeline: async (id, pipeline) => {
    const updated = await projectService.updatePipeline(id, pipeline);
    if (!updated) return;
    set((state) => ({ projects: state.projects.map((item) => (item.id === id ? updated : item)) }));
  },

  deleteProject: async (id, cascadeAction = 'moveToInbox') => {
    if (cascadeAction === 'moveToInbox') {
      await projectService.delete(id);
    } else {
      await projectService.delete(id, cascadeAction);
    }

    const activeProjectId = get().activeProjectId === id ? null : get().activeProjectId;
    if (get().activeProjectId === id) {
      await projectService.setActiveProjectId(null);
    }

    const config = useConfigStore.getState().config;
    const profiles = normalizeSpeakerProfiles(config.speakerProfiles);
    if (profiles.some((p) => p.projectIds?.includes(id))) {
      const nextProfiles = profiles.map((p) => {
        if (!p.projectIds?.includes(id)) return p;
        return {
          ...p,
          projectIds: p.projectIds.filter((pid) => pid !== id),
        };
      });
      useConfigStore.getState().setConfig({ speakerProfiles: nextProfiles });
    }

    set((state) => ({
      projects: state.projects.filter((item) => item.id !== id),
      activeProjectId,
    }));
  },

  moveItemsToProject: async (historyIds, projectId) => {
    if (historyIds.length === 0) return;
    await historyService.updateProjectAssignments(historyIds, projectId);
  },

  setActiveProjectId: async (projectId) => {
    try {
      await projectService.setActiveProjectId(projectId);
      set({ activeProjectId: projectId, error: null });
    } catch (error) {
      logger.error('[Projects] Failed to persist active project id:', error);
      set({
        activeProjectId: projectId,
        error: extractErrorMessage(error) || 'Failed to persist active project',
      });
    }
  },

  assignHistoryItems: async (historyIds, projectId) => {
    if (historyIds.length === 0) {
      return;
    }

    await historyService.updateProjectAssignments(historyIds, projectId);
  },

  reorderProjects: async (projectIds) => {
    const { projects } = get();
    const projectMap = new Map(projects.map((p) => [p.id, p]));
    const nextProjects = projectIds
      .map((id) => projectMap.get(id))
      .filter((p): p is ProjectRecord => !!p);

    if (nextProjects.length < projects.length) {
      const addedIds = new Set(projectIds);
      projects.forEach((p) => {
        if (!addedIds.has(p.id)) {
          nextProjects.push(p);
        }
      });
    }

    set({ projects: nextProjects });
    await projectService.reorder(nextProjects.map((p) => p.id));
  },

  getActiveProject: () => {
    const { activeProjectId, projects } = get();
    if (!activeProjectId) {
      return null;
    }
    return projects.find((item) => item.id === activeProjectId) || null;
  },

  getProjectById: (projectId) => {
    if (!projectId) {
      return null;
    }
    return get().projects.find((item) => item.id === projectId) || null;
  },
}));
