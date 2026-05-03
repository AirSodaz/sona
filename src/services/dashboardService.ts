import { getDashboardSnapshot } from './tauri/dashboard';
import type { DashboardSnapshot } from '../types/dashboard';

class DashboardService {
  async getFastSnapshot(): Promise<DashboardSnapshot> {
    return getDashboardSnapshot({ deep: false });
  }

  async getDeepSnapshot(): Promise<DashboardSnapshot> {
    return getDashboardSnapshot({ deep: true });
  }
}

export const dashboardService = new DashboardService();
