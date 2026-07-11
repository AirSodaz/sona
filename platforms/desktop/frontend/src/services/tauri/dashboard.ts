import type { DashboardSnapshot } from '../../types/dashboard';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export type DashboardSnapshotRequest = {
  deep: boolean;
};

export async function getDashboardSnapshot(
  request: DashboardSnapshotRequest,
): Promise<DashboardSnapshot> {
  return invokeTauri(TauriCommand.dashboard.getSnapshot, { request });
}
