export interface DialogFilter {
  name: string;
  extensions: string[];
}

export interface OpenFileOptions {
  title?: string;
  defaultPath?: string;
  multiple?: boolean;
  directory?: boolean;
  filters?: DialogFilter[];
}

export interface SaveFileOptions {
  title?: string;
  defaultPath?: string;
  filters?: DialogFilter[];
}

export interface IAssetPort {
  /**
   * Converts a native file path or local URI to a web-viewable audio URL.
   */
  convertAudioSrc(pathOrUri: string): string;
}

export interface IDialogPort {
  /**
   * Opens an OS file/folder picker dialog.
   */
  openFile(options?: OpenFileOptions): Promise<string[] | string | null>;

  /**
   * Opens an OS save file dialog.
   */
  saveFile(options?: SaveFileOptions): Promise<string | null>;
}

export interface MkdirOptions {
  recursive?: boolean;
}

export interface RemoveOptions {
  recursive?: boolean;
}

export interface IFileSystemPort {
  /**
   * Checks whether a file or directory exists.
   */
  exists(path: string): Promise<boolean>;

  /**
   * Creates a directory.
   */
  mkdir(path: string, options?: MkdirOptions): Promise<void>;

  /**
   * Deletes a file or directory.
   */
  remove(path: string, options?: RemoveOptions): Promise<void>;

  /**
   * Writes string content to a file.
   */
  writeText(path: string, content: string): Promise<void>;

  /**
   * Reads string content from a file.
   */
  readText(path: string): Promise<string>;

  /**
   * Writes binary data to a file (optional).
   */
  writeFile?(path: string, data: Uint8Array): Promise<void>;

  /**
   * Reads binary data from a file (optional).
   */
  readFile?(path: string): Promise<Uint8Array>;
}

export interface IPathPort {
  /**
   * Returns the app-specific local data directory path.
   */
  appLocalDataDir(): Promise<string>;

  /**
   * Returns the OS temporary directory path.
   */
  tempDir(): Promise<string>;

  /**
   * Joins path segments using the platform separator.
   */
  join(...paths: string[]): Promise<string> | string;
}

export interface IAppLifecyclePort {
  /**
   * Opens a URL in the default browser.
   */
  openUrl(url: string): Promise<void>;

  /**
   * Relaunches the application.
   */
  relaunch(): Promise<void>;

  /**
   * Opens the application log folder in the OS file manager.
   */
  openLogFolder(): Promise<void>;
}

export interface IPlatformPorts {
  assets: IAssetPort;
  dialog: IDialogPort;
  fs: IFileSystemPort;
  path: IPathPort;
  lifecycle: IAppLifecyclePort;
}
