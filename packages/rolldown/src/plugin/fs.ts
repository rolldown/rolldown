import fsp from 'node:fs/promises';

export interface RolldownFsModule {
  appendFile(
    path: string,
    data: string | Uint8Array,
    options?: {
      encoding?: BufferEncoding | null;
      mode?: string | number;
      flag?: string | number;
    },
  ): Promise<void>;

  copyFile(
    source: string,
    destination: string,
    mode?: string | number,
  ): Promise<void>;

  mkdir(
    path: string,
    options?: { recursive?: boolean; mode?: string | number },
  ): Promise<void>;

  mkdtemp(prefix: string): Promise<string>;

  readdir(path: string, options?: { withFileTypes?: false }): Promise<string[]>;
  readdir(
    path: string,
    options?: { withFileTypes: true },
  ): Promise<RolldownDirectoryEntry[]>;

  readFile(
    path: string,
    options?: { encoding?: null; flag?: string | number; signal?: AbortSignal },
  ): Promise<Uint8Array>;
  readFile(
    path: string,
    options?: {
      encoding: BufferEncoding;
      flag?: string | number;
      signal?: AbortSignal;
    },
  ): Promise<string>;

  realpath(path: string): Promise<string>;

  rename(oldPath: string, newPath: string): Promise<void>;

  rmdir(path: string, options?: { recursive?: boolean }): Promise<void>;

  stat(path: string): Promise<RolldownFileStats>;

  lstat(path: string): Promise<RolldownFileStats>;

  unlink(path: string): Promise<void>;

  writeFile(
    path: string,
    data: string | Uint8Array,
    options?: {
      encoding?: BufferEncoding | null;
      mode?: string | number;
      flag?: string | number;
    },
  ): Promise<void>;
}

export type BufferEncoding =
  | 'ascii'
  | 'utf8'
  | 'utf16le'
  | 'ucs2'
  | 'base64'
  | 'base64url'
  | 'latin1'
  | 'binary'
  | 'hex';

export interface RolldownDirectoryEntry {
  isFile(): boolean;
  isDirectory(): boolean;
  isSymbolicLink(): boolean;
  name: string;
}

export interface RolldownFileStats {
  isFile(): boolean;
  isDirectory(): boolean;
  isSymbolicLink(): boolean;
  size: number;
  mtime: Date;
  ctime: Date;
  atime: Date;
  birthtime: Date;
}

export const fsModule: RolldownFsModule = {
  appendFile: fsp.appendFile,
  copyFile: fsp.copyFile,
  mkdir: fsp.mkdir as RolldownFsModule['mkdir'],
  mkdtemp: fsp.mkdtemp,
  readdir: fsp.readdir,
  readFile: fsp.readFile as RolldownFsModule['readFile'],
  realpath: fsp.realpath,
  rename: fsp.rename,
  rmdir: fsp.rmdir,
  stat: fsp.stat,
  lstat: fsp.lstat,
  unlink: fsp.unlink,
  writeFile: fsp.writeFile,
};
