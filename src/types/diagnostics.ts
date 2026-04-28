import type { SettingsTab } from '../hooks/useSettingsLogic';

export type DiagnosticStatus = 'ready' | 'warning' | 'missing' | 'failed' | 'info';

export type DiagnosticAction =
  | {
      kind: 'open_settings';
      label: string;
      settingsTab: SettingsTab;
    }
  | {
      kind: 'request_microphone_permission';
      label: string;
    }
  | {
      kind: 'retry_voice_typing_warmup';
      label: string;
    }
  | {
      kind: 'run_first_run_setup';
      label: string;
    }
  | {
      kind: 'open_log_folder';
      label: string;
    };

export interface DiagnosticCheck {
  id: string;
  title: string;
  description: string;
  status: DiagnosticStatus;
  action?: DiagnosticAction;
  meta?: string;
}

export interface DiagnosticSection {
  id: string;
  title: string;
  description?: string;
  checks: DiagnosticCheck[];
}

export interface DiagnosticOverviewCard {
  id: string;
  title: string;
  description: string;
  status: DiagnosticStatus;
  action?: DiagnosticAction;
}

export interface RuntimeEnvironmentStatus {
  ffmpegPath: string;
  ffmpegExists: boolean;
  logDirPath: string;
}

export interface DiagnosticsSnapshot {
  scannedAt: string;
  overview: DiagnosticOverviewCard[];
  sections: DiagnosticSection[];
  runtimeEnvironment: RuntimeEnvironmentStatus;
}
