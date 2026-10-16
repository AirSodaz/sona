export interface McpStatus {
  enabled: boolean;
  running: boolean;
  endpoint: string;
  activeConnections: number;
  binaryPath?: string | null;
  binaryExists: boolean;
  permissionMode: string;
}

export interface McpActivityLogEntry {
  id: string;
  timestamp: number;
  method: string;
  category: string;
  status: 'success' | 'error' | 'blocked';
  durationMs: number;
  paramsSummary?: string | null;
  errorMessage?: string | null;
}

export interface McpToolCatalogItem {
  name: string;
  title: string;
  description: string;
  category: string;
  allowed: boolean;
}

export interface McpClientGuide {
  id: string;
  name: string;
  configPath: string;
  configContent: string;
  format: 'json' | 'bash' | 'toml';
}

export interface McpClientConfigMap {
  binaryPath: string;
  binaryExists: boolean;
  endpoint: string;
  guides: McpClientGuide[];
}

export interface McpMutationNotice {
  id?: string;
  method: string;
  category: string;
  summary?: string | null;
  timestamp: number;
}
