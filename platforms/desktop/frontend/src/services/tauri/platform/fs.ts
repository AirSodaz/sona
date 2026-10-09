import {
  BaseDirectory,
  exists,
  mkdir,
  readFile,
  readTextFile,
  remove,
  writeFile,
  writeTextFile,
} from '@tauri-apps/plugin-fs';

export type {
  ExistsOptions,
  MkdirOptions,
  RemoveOptions,
  WriteFileOptions,
} from '@tauri-apps/plugin-fs';

export { BaseDirectory, exists, mkdir, readFile, readTextFile, remove, writeFile, writeTextFile };
