import { invoke } from '@tauri-apps/api/core';
import type { RuntimePathStatus } from '../types/runtime';
import { logger } from '../utils/logger';

function createUnknownStatus(path: string, error?: string | null): RuntimePathStatus {
  return {
    path,
    kind: 'unknown',
    error: error ?? null,
  };
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export async function getPathStatusMap(paths: string[]): Promise<Record<string, RuntimePathStatus>> {
  const normalizedPaths = paths
    .filter((path): path is string => typeof path === 'string')
    .map((path) => path.trim())
    .filter((path) => path.length > 0);
  const uniquePaths = [...new Set(normalizedPaths)];

  if (uniquePaths.length === 0) {
    return {};
  }

  try {
    const result = await invoke<RuntimePathStatus[]>('get_path_statuses', {
      paths: uniquePaths,
    });

    const pathStatusMap = Object.fromEntries(
      uniquePaths.map((path) => [path, createUnknownStatus(path)]),
    ) as Record<string, RuntimePathStatus>;

    result.forEach((status) => {
      if (!status?.path || !(status.path in pathStatusMap)) {
        return;
      }

      pathStatusMap[status.path] = {
        path: status.path,
        kind: status.kind,
        error: status.error ?? null,
      };
    });

    return pathStatusMap;
  } catch (error) {
    const errorMessage = getErrorMessage(error);
    logger.error('[PathStatus] Failed to query runtime path statuses:', error);

    return Object.fromEntries(
      uniquePaths.map((path) => [path, createUnknownStatus(path, errorMessage)]),
    ) as Record<string, RuntimePathStatus>;
  }
}

export function isRuntimePathAvailable(pathStatus?: RuntimePathStatus): boolean {
  return pathStatus?.kind === 'file' || pathStatus?.kind === 'directory';
}

export function isRuntimePathFile(pathStatus?: RuntimePathStatus): boolean {
  return pathStatus?.kind === 'file';
}
