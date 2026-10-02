import { createHash } from 'node:crypto';
import {
  closeSync,
  constants,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  readSync,
  readdirSync,
  rmSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join } from 'node:path';

const HASH_BUFFER_SIZE = 1024 * 1024;

interface ArtifactSnapshot {
  backupPath: string;
}

export interface BuildArtifactTransaction {
  commit(): void;
  rollback(): void;
}

export interface BuildArtifactSelection {
  names?: readonly string[];
  prefixes?: readonly string[];
}

export const BINDING_BUILD_ARTIFACT_SELECTION = {
  names: [
    'binding.cjs',
    'binding.d.cts',
    'browser.js',
    'wasi-worker.mjs',
    'wasi-worker-browser.mjs',
  ],
  prefixes: ['rolldown-binding.'],
} as const satisfies BuildArtifactSelection;

export function beginBuildArtifactTransaction(
  outputDir: string,
  selection: BuildArtifactSelection,
): BuildArtifactTransaction {
  const artifactNames = new Set(selection.names);
  const artifactPrefixes = selection.prefixes ?? [];
  const isManagedArtifact = (name: string) =>
    artifactNames.has(name) || artifactPrefixes.some((prefix) => name.startsWith(prefix));
  const backupDir = mkdtempSync(join(tmpdir(), 'rolldown-binding-build-'));
  const snapshots = new Map<string, ArtifactSnapshot>();

  try {
    for (const artifactPath of listArtifacts(outputDir, isManagedArtifact)) {
      const stat = lstatSync(artifactPath);
      if (!stat.isFile()) {
        throw new Error(`Refusing to snapshot non-file build artifact ${artifactPath}`);
      }
      const backupPath = join(backupDir, basename(artifactPath));
      copyFileSync(artifactPath, backupPath, constants.COPYFILE_FICLONE);
      snapshots.set(artifactPath, { backupPath });
    }
  } catch (error) {
    rmSync(backupDir, { force: true, recursive: true });
    throw error;
  }

  let active = true;

  return {
    commit() {
      if (!active) return;
      active = false;
      try {
        rmSync(backupDir, { force: true, recursive: true });
      } catch {
        // A stale temporary backup must not invalidate an otherwise successful build.
      }
    },
    rollback() {
      if (!active) return;
      active = false;
      const errors: unknown[] = [];

      let currentArtifacts: string[] = [];
      try {
        currentArtifacts = listArtifacts(outputDir, isManagedArtifact);
      } catch (error) {
        errors.push(
          new Error(`Failed to inspect build artifacts in ${outputDir}`, {
            cause: error,
          }),
        );
      }

      for (const artifactPath of currentArtifacts) {
        if (snapshots.has(artifactPath)) continue;
        try {
          removeArtifact(artifactPath);
        } catch (error) {
          errors.push(error);
        }
      }

      for (const [artifactPath, snapshot] of snapshots) {
        try {
          if (existsSync(artifactPath)) {
            const stat = lstatSync(artifactPath);
            if (stat.isFile() && artifactMatchesSnapshot(artifactPath, snapshot)) {
              continue;
            }
            removeArtifact(artifactPath);
          }
          mkdirSync(outputDir, { recursive: true });
          copyFileSync(snapshot.backupPath, artifactPath, constants.COPYFILE_FICLONE);
        } catch (error) {
          errors.push(
            new Error(`Failed to restore build artifact ${artifactPath}`, {
              cause: error,
            }),
          );
        }
      }

      if (errors.length === 0) {
        try {
          rmSync(backupDir, { force: true, recursive: true });
        } catch (error) {
          errors.push(
            new Error(`Failed to remove build artifact backup ${backupDir}`, {
              cause: error,
            }),
          );
        }
      }

      if (errors.length > 0) {
        throw new AggregateError(
          errors,
          `Failed to roll back build artifacts; backup retained at ${backupDir}`,
        );
      }
    },
  };
}

function listArtifacts(outputDir: string, isManagedArtifact: (name: string) => boolean): string[] {
  if (!existsSync(outputDir)) return [];
  return readdirSync(outputDir)
    .filter(isManagedArtifact)
    .map((name) => join(outputDir, name));
}

function removeArtifact(artifactPath: string): void {
  let stat: ReturnType<typeof lstatSync>;
  try {
    stat = lstatSync(artifactPath);
  } catch (error) {
    if (isNodeError(error) && error.code === 'ENOENT') return;
    throw error;
  }
  if (!stat.isFile() && !stat.isSymbolicLink()) {
    throw new Error(`Refusing to remove non-file build artifact ${artifactPath}`);
  }
  rmSync(artifactPath, { force: true });
}

function artifactMatchesSnapshot(artifactPath: string, snapshot: ArtifactSnapshot): boolean {
  try {
    const artifactStat = lstatSync(artifactPath);
    const snapshotStat = lstatSync(snapshot.backupPath);
    return (
      artifactStat.size === snapshotStat.size &&
      hashFile(artifactPath) === hashFile(snapshot.backupPath)
    );
  } catch {
    return false;
  }
}

function hashFile(filePath: string): string {
  const hash = createHash('sha256');
  const buffer = Buffer.allocUnsafe(HASH_BUFFER_SIZE);
  const file = openSync(filePath, 'r');
  try {
    let bytesRead = 0;
    do {
      bytesRead = readSync(file, buffer, 0, buffer.length, null);
      if (bytesRead > 0) {
        hash.update(buffer.subarray(0, bytesRead));
      }
    } while (bytesRead > 0);
  } finally {
    closeSync(file);
  }
  return hash.digest('hex');
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && 'code' in error;
}
