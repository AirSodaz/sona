import type {
  IAppLifecyclePort,
  IAssetPort,
  IDialogPort,
  IFileSystemPort,
  IPathPort,
  IPlatformPorts,
  MkdirOptions,
  OpenFileOptions,
  RemoveOptions,
  SaveFileOptions,
} from '../../types/ports';

export class MockAssetPort implements IAssetPort {
  constructor(private readonly prefix: string = 'mock-asset://') {}

  convertAudioSrc(pathOrUri: string): string {
    return `${this.prefix}${pathOrUri}`;
  }
}

export interface DialogCallRecord {
  method: 'openFile' | 'saveFile';
  options?: unknown;
}

export class MockDialogPort implements IDialogPort {
  private nextOpenResult: string[] | string | null = null;
  private nextSaveResult: string | null = null;
  private calls: DialogCallRecord[] = [];

  setNextOpenResult(result: string[] | string | null): void {
    this.nextOpenResult = result;
  }

  setNextSaveResult(result: string | null): void {
    this.nextSaveResult = result;
  }

  getCalls(): readonly DialogCallRecord[] {
    return [...this.calls];
  }

  clearCalls(): void {
    this.calls = [];
  }

  async openFile(options?: OpenFileOptions): Promise<string[] | string | null> {
    this.calls.push({ method: 'openFile', options });
    return this.nextOpenResult;
  }

  async saveFile(options?: SaveFileOptions): Promise<string | null> {
    this.calls.push({ method: 'saveFile', options });
    return this.nextSaveResult;
  }
}

export class InMemoryFileSystemPort implements IFileSystemPort {
  private files = new Map<string, string | Uint8Array>();
  private directories = new Set<string>();

  async exists(path: string): Promise<boolean> {
    return this.files.has(path) || this.directories.has(path);
  }

  async mkdir(path: string, _options?: MkdirOptions): Promise<void> {
    this.directories.add(path);
  }

  async remove(path: string, _options?: RemoveOptions): Promise<void> {
    this.files.delete(path);
    this.directories.delete(path);
    for (const fileKey of Array.from(this.files.keys())) {
      if (fileKey.startsWith(`${path}/`) || fileKey.startsWith(`${path}\\`)) {
        this.files.delete(fileKey);
      }
    }
    for (const dirKey of Array.from(this.directories.keys())) {
      if (dirKey.startsWith(`${path}/`) || dirKey.startsWith(`${path}\\`)) {
        this.directories.delete(dirKey);
      }
    }
  }

  async writeText(path: string, content: string): Promise<void> {
    this.files.set(path, content);
  }

  async readText(path: string): Promise<string> {
    const content = this.files.get(path);
    if (content === undefined) {
      throw new Error(`[InMemoryFileSystemPort] File not found: ${path}`);
    }
    return typeof content === 'string' ? content : new TextDecoder().decode(content);
  }

  async writeFile(path: string, data: Uint8Array): Promise<void> {
    this.files.set(path, data);
  }

  async readFile(path: string): Promise<Uint8Array> {
    const content = this.files.get(path);
    if (content === undefined) {
      throw new Error(`[InMemoryFileSystemPort] File not found: ${path}`);
    }
    return typeof content === 'string' ? new TextEncoder().encode(content) : content;
  }

  setFile(path: string, content: string | Uint8Array): void {
    this.files.set(path, content);
  }

  getFile(path: string): string | Uint8Array | undefined {
    return this.files.get(path);
  }

  clear(): void {
    this.files.clear();
    this.directories.clear();
  }
}

export class MockPathPort implements IPathPort {
  constructor(
    private readonly localDataDir: string = '/mock/app-data',
    private readonly temporaryDir: string = '/mock/temp'
  ) {}

  async appLocalDataDir(): Promise<string> {
    return this.localDataDir;
  }

  async tempDir(): Promise<string> {
    return this.temporaryDir;
  }

  join(...paths: string[]): string {
    return paths
      .map((part) => part.replace(/[/\\]+$/, ''))
      .filter((part) => part.length > 0)
      .join('/')
      .replace(/\/+/g, '/');
  }
}

export class MockAppLifecyclePort implements IAppLifecyclePort {
  openedUrls: string[] = [];
  relaunchCount = 0;
  openLogFolderCount = 0;

  async openUrl(url: string): Promise<void> {
    this.openedUrls.push(url);
  }

  async relaunch(): Promise<void> {
    this.relaunchCount += 1;
  }

  async openLogFolder(): Promise<void> {
    this.openLogFolderCount += 1;
  }

  clear(): void {
    this.openedUrls = [];
    this.relaunchCount = 0;
    this.openLogFolderCount = 0;
  }
}

export function createMockPlatformPorts(overrides?: Partial<IPlatformPorts>): IPlatformPorts {
  return {
    assets: overrides?.assets ?? new MockAssetPort(),
    dialog: overrides?.dialog ?? new MockDialogPort(),
    fs: overrides?.fs ?? new InMemoryFileSystemPort(),
    path: overrides?.path ?? new MockPathPort(),
    lifecycle: overrides?.lifecycle ?? new MockAppLifecyclePort(),
  };
}
