import type {
  IAppLifecyclePort,
  IAssetPort,
  IDialogPort,
  IPathPort,
  IPlatformPorts,
  OpenFileOptions,
  SaveFileOptions,
} from '../../types/ports';
import { InMemoryFileSystemPort } from '../mock/MockPlatformPorts';
import { UnsupportedCapabilityError } from './errors';

export class BrowserAssetPort implements IAssetPort {
  convertAudioSrc(pathOrUri: string): string {
    if (
      pathOrUri.startsWith('blob:') ||
      pathOrUri.startsWith('data:') ||
      pathOrUri.startsWith('http:') ||
      pathOrUri.startsWith('https:')
    ) {
      return pathOrUri;
    }
    return pathOrUri;
  }
}

export class BrowserDialogPort implements IDialogPort {
  async openFile(options?: OpenFileOptions): Promise<string[] | string | null> {
    if (typeof document === 'undefined') {
      return null;
    }

    const { promise, resolve } =
      typeof Promise.withResolvers === 'function'
        ? Promise.withResolvers<string[] | string | null>()
        : (() => {
            let res!: (value: string[] | string | null) => void;
            const p = new Promise<string[] | string | null>((r) => {
              res = r;
            });
            return { promise: p, resolve: res };
          })();

    const input = document.createElement('input');
    input.type = 'file';
    input.style.display = 'none';

    if (options?.multiple) {
      input.multiple = true;
    }

    if (options?.filters && options.filters.length > 0) {
      const extensions = options.filters.flatMap((filter) =>
        filter.extensions.map((ext) => (ext.startsWith('.') ? ext : `.${ext}`))
      );
      input.accept = extensions.join(',');
    }

    const cleanup = () => {
      if (input.parentNode) {
        input.parentNode.removeChild(input);
      }
    };

    input.onchange = () => {
      try {
        if (!input.files || input.files.length === 0) {
          resolve(null);
          return;
        }

        const files = Array.from(input.files).map((file) => file.name);
        if (options?.multiple) {
          resolve(files);
        } else {
          resolve(files[0] ?? null);
        }
      } finally {
        cleanup();
      }
    };

    input.oncancel = () => {
      try {
        resolve(null);
      } finally {
        cleanup();
      }
    };

    document.body.appendChild(input);
    input.click();

    return promise;
  }

  async saveFile(options?: SaveFileOptions): Promise<string | null> {
    return options?.defaultPath ?? 'download';
  }
}

export class BrowserPathPort implements IPathPort {
  constructor(
    private readonly localDataDir: string = '/web/data',
    private readonly temporaryDir: string = '/web/temp'
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

export class BrowserAppLifecyclePort implements IAppLifecyclePort {
  async openUrl(url: string): Promise<void> {
    if (typeof window !== 'undefined') {
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  }

  async relaunch(): Promise<void> {
    if (typeof window !== 'undefined') {
      window.location.reload();
    }
  }

  async openLogFolder(): Promise<void> {
    throw new UnsupportedCapabilityError(
      'open_log_folder',
      'Log folder cannot be opened natively in browser sandbox.'
    );
  }
}

export function createBrowserPlatformPorts(overrides?: Partial<IPlatformPorts>): IPlatformPorts {
  return {
    assets: overrides?.assets ?? new BrowserAssetPort(),
    dialog: overrides?.dialog ?? new BrowserDialogPort(),
    fs: overrides?.fs ?? new InMemoryFileSystemPort(),
    path: overrides?.path ?? new BrowserPathPort(),
    lifecycle: overrides?.lifecycle ?? new BrowserAppLifecyclePort(),
  };
}
