import { create } from 'zustand';
import type { AppConfig } from '../types/config';
import type { ProjectDefaults, ProjectRecord } from '../types/project';
import { buildProjectDefaultsFromConfig } from '../types/project';
import { historyService } from '../services/historyService';
import { projectService } from '../services/projectService';
import { logger } from '../utils/logger';

interface CreateProjectInput {
  name: string;
  description?: string;
  defaults?: ProjectDefaults;
}

interface ProjectState {
  projects: ProjectRecord[];
  activeProjectId: string | null;
  isLoading: boolean;
  error: string | null;
  loadProjects: () => Promise<void>;
  createProject: (input: CreateProjectInput, globalConfig: AppConfig) => Promise<ProjectRecord | null>;
  updateProject: (id: string, updates: Partial<Pick<ProjectRecord, 'name' | 'description' | 'defaults'>>) => Promise<void>;
  updateProjectDefaults: (id: string, updates: Partial<ProjectDefaults>) => Promise<void>;
  deleteProject: (id: string) => Promise<void>;
  setActiveProjectId: (projectId: string | null) => Promise<void>;
  assignHistoryItems: (historyIds: string[], projectId: string | null) => Promise<void>;
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

      const isValidActiveProject = !activeProjectId || projects.some((item) => item.id === activeProjectId);
      const normalizedActiveProjectId = isValidActiveProject ? activeProjectId : null;

      if (activeProjectId && !isValidActiveProject) {
        await projectService.setActiveProjectId(null);
      }

      set({
        projects,
        activeProjectId: normalizedActiveProjectId,
        isLoading: false,
      });
    } catch (error: any) {
      set({
        error: error?.message || 'Failed to load projects',
        isLoading: false,
      });
    }
  },

  createProject: async (input, globalConfig) => {
    const defaults = input.defaults || buildProjectDefaultsFromConfig(globalConfig);
    const project = await projectService.create({
      name: input.name,
      description: input.description || '',
      defaults,
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

  updateProjectDefaults: async (id, updates) => {
    const project = get().projects.find((item) => item.id === id);
    if (!project) {
      return;
    }

    await get().updateProject(id, {
      defaults: {
        ...project.defaults,
        ...updates,
      },
    });
  },

  deleteProject: async (id) => {
    await historyService.updateProjectAssignmentsByCurrentProject(id, null);
    await projectService.delete(id);

    const activeProjectId = get().activeProjectId === id ? null : get().activeProjectId;
    if (get().activeProjectId === id) {
      await projectService.setActiveProjectId(null);
    }

    set((state) => ({
      projects: state.projects.filter((item) => item.id !== id),
      activeProjectId,
    }));
  },

  setActiveProjectId: async (projectId) => {
    try {
      await projectService.setActiveProjectId(projectId);
      set({ activeProjectId: projectId, error: null });
    } catch (error: any) {
      logger.error('[Projects] Failed to persist active project id:', error);
      set({
        activeProjectId: projectId,
        error: error?.message || 'Failed to persist active project',
      });
    }
  },

  assignHistoryItems: async (historyIds, projectId) => {
    if (historyIds.length === 0) {
      return;
    }

    await historyService.updateProjectAssignments(historyIds, projectId);
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
