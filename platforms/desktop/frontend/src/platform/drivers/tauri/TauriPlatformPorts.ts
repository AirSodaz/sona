import { TauriCommand } from '../../../services/tauri/commands';
import { invokeTauri } from '../../../services/tauri/invoke';
import { convertManagedAudioFileSrc } from '../../../services/tauri/platform/assets';
import { openDialog, saveDialog } from '../../../services/tauri/platform/dialog';
import {
  exists,
  mkdir,
  readFile,
  readTextFile,
  remove,
  writeFile,
  writeTextFile,
} from '../../../services/tauri/platform/fs';
import { openUrl } from '../../../services/tauri/platform/opener';
import { appLocalDataDir, join, tempDir } from '../../../services/tauri/platform/path';
import { relaunch } from '../../../services/tauri/platform/process';
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

export class TauriAssetPort implements IAssetPort {
  convertAudioSrc(pathOrUri: string): string {
    return convertManagedAudioFileSrc(pathOrUri);
  }
}

export class TauriDialogPort implements IDialogPort {
  async openFile(options?: OpenFileOptions): Promise<string[] | string | null> {
    return openDialog(options);
  }

  async saveFile(options?: SaveFileOptions): Promise<string | null> {
    return saveDialog(options);
  }
}

export class TauriFileSystemPort implements IFileSystemPort {
  async exists(path: string): Promise<boolean> {
    return exists(path);
  }

  async mkdir(path: string, options?: MkdirOptions): Promise<void> {
    await mkdir(path, options);
  }

  async remove(path: string, options?: RemoveOptions): Promise<void> {
    await remove(path, options);
  }

  async writeText(path: string, content: string): Promise<void> {
    await writeTextFile(path, content);
  }

  async readText(path: string): Promise<string> {
    return readTextFile(path);
  }

  async writeFile(path: string, data: Uint8Array): Promise<void> {
    await writeFile(path, data);
  }

  async readFile(path: string): Promise<Uint8Array> {
    return readFile(path);
  }
}

export class TauriPathPort implements IPathPort {
  async appLocalDataDir(): Promise<string> {
    return appLocalDataDir();
  }

  async tempDir(): Promise<string> {
    return tempDir();
  }

  async join(...paths: string[]): Promise<string> {
    return join(...paths);
  }
}

export class TauriAppLifecyclePort implements IAppLifecyclePort {
  async openUrl(url: string): Promise<void> {
    await openUrl(url);
  }

  async relaunch(): Promise<void> {
    await relaunch();
  }

  async openLogFolder(): Promise<void> {
    await invokeTauri(TauriCommand.app.openLogFolder);
  }
}

export function createTauriPlatformPorts(): IPlatformPorts {
  return {
    assets: new TauriAssetPort(),
    dialog: new TauriDialogPort(),
    fs: new TauriFileSystemPort(),
    path: new TauriPathPort(),
    lifecycle: new TauriAppLifecyclePort(),
  };
}
