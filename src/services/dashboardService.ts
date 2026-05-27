import { getDashboardSnapshot } from './tauri/dashboard';
import type { DashboardSnapshot } from '../types/dashboard';

export interface DashboardServicePorts {
  getDashboardSnapshot: typeof getDashboardSnapshot;
}

export class DashboardService {
  constructor(private readonly ports: DashboardServicePorts) {}

  async getFastSnapshot(): Promise<DashboardSnapshot> {
    return this.ports.getDashboardSnapshot({ deep: false });
  }

  async getDeepSnapshot(): Promise<DashboardSnapshot> {
    return this.ports.getDashboardSnapshot({ deep: true });
  }
}

export function createDashboardService(ports: DashboardServicePorts): DashboardService {
  return new DashboardService(ports);
}

export const dashboardService = createDashboardService({
  getDashboardSnapshot,
});
