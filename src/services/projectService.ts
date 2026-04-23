import { readTextFile, writeTextFile, exists, mkdir, BaseDirectory } from '@tauri-apps/plugin-fs';
import { v4 as uuidv4 } from 'uuid';
import { settingsStore, STORE_KEY_ACTIVE_PROJECT } from './storageService';
import { logger } from '../utils/logger';
import type { ProjectDefaults, ProjectRecord } from '../types/project';
import { normalizeProjectRecord } from '../types/project';

const PROJECTS_DIR = 'projects';
const INDEX_FILE = 'index.json';

async function ensureProjectsIndex() {
  const dirExists = await exists(PROJECTS_DIR, { baseDir: BaseDirectory.AppLocalData });
  if (!dirExists) {
    await mkdir(PROJECTS_DIR, { baseDir: BaseDirectory.AppLocalData, recursive: true });
  }

  const indexPath = `${PROJECTS_DIR}/${INDEX_FILE}`;
  const indexExists = await exists(indexPath, { baseDir: BaseDirectory.AppLocalData });
  if (!indexExists) {
    await writeTextFile(indexPath, '[]', { baseDir: BaseDirectory.AppLocalData });
  }
}

async function writeProjects(projects: ProjectRecord[]) {
  await writeTextFile(
    `${PROJECTS_DIR}/${INDEX_FILE}`,
    JSON.stringify(projects, null, 2),
    { baseDir: BaseDirectory.AppLocalData },
  );
}

export const projectService = {
  async init(): Promise<void> {
    await ensureProjectsIndex();
  },

  async getAll(): Promise<ProjectRecord[]> {
    try {
      await ensureProjectsIndex();
      const content = await readTextFile(`${PROJECTS_DIR}/${INDEX_FILE}`, {
        baseDir: BaseDirectory.AppLocalData,
      });
      const parsed = JSON.parse(content) as Partial<ProjectRecord>[];
      return parsed
        .map((item) => normalizeProjectRecord(item))
        .sort((a, b) => b.updatedAt - a.updatedAt);
    } catch (error) {
      logger.error('[Projects] Failed to load projects:', error);
      return [];
    }
  },

  async create(input: { name: string; description?: string; defaults: ProjectDefaults }): Promise<ProjectRecord> {
    const now = Date.now();
    const project = normalizeProjectRecord({
      id: uuidv4(),
      name: input.name,
      description: input.description || '',
      createdAt: now,
      updatedAt: now,
      defaults: input.defaults,
    });

    const projects = await this.getAll();
    projects.unshift(project);
    await writeProjects(projects);
    return project;
  },

  async update(
    id: string,
    updates: Partial<Pick<ProjectRecord, 'name' | 'description' | 'defaults'>>,
  ): Promise<ProjectRecord | null> {
    const projects = await this.getAll();
    const project = projects.find((item) => item.id === id);
    if (!project) {
      return null;
    }

    const updated = normalizeProjectRecord({
      ...project,
      ...updates,
      defaults: {
        ...project.defaults,
        ...(updates.defaults || {}),
      },
      updatedAt: Date.now(),
    });

    const nextProjects = projects.map((item) => (item.id === id ? updated : item));
    await writeProjects(nextProjects);
    return updated;
  },

  async delete(id: string): Promise<void> {
    const projects = await this.getAll();
    const nextProjects = projects.filter((item) => item.id !== id);
    await writeProjects(nextProjects);
  },

  async getActiveProjectId(): Promise<string | null> {
    try {
      const value = await settingsStore.get<string | null>(STORE_KEY_ACTIVE_PROJECT);
      return typeof value === 'string' && value.trim() ? value : null;
    } catch (error) {
      logger.error('[Projects] Failed to load active project id:', error);
      return null;
    }
  },

  async setActiveProjectId(projectId: string | null): Promise<void> {
    try {
      await settingsStore.set(STORE_KEY_ACTIVE_PROJECT, projectId);
      await settingsStore.save();
    } catch (error) {
      logger.error('[Projects] Failed to persist active project id:', error);
      throw error;
    }
  },
};
