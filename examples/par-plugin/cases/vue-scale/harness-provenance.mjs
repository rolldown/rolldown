import { createHash } from 'node:crypto';
import { lstat, readFile, readlink, readdir } from 'node:fs/promises';
import nodePath from 'node:path';

const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const roots = [
  nodePath.join(repositoryRoot, 'examples/par-plugin/cases/vue-scale'),
  nodePath.join(repositoryRoot, 'examples/par-plugin/parallel-vue-plugin'),
];
const explicitFiles = [
  nodePath.join(repositoryRoot, 'examples/par-plugin/package.json'),
  nodePath.join(repositoryRoot, 'pnpm-lock.yaml'),
];
const ignoredDirectories = new Set(['.corpus', '.results', 'evidence']);
const compareUtf8 = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const sha256 = (value) => createHash('sha256').update(value).digest('hex');

export async function captureHarnessSourceManifest() {
  const entries = [];
  for (const root of roots) await walk(root, entries);
  for (const path of explicitFiles) await captureFile(path, entries);
  entries.sort((left, right) => compareUtf8(left.path, right.path));
  const aggregate = createHash('sha256');
  for (const entry of entries) {
    aggregate.update(entry.path);
    aggregate.update('\0');
    aggregate.update(entry.kind);
    aggregate.update('\0');
    aggregate.update(String(entry.bytes));
    aggregate.update('\0');
    aggregate.update(entry.sha256);
    aggregate.update('\n');
  }
  return {
    algorithm:
      'SHA-256 over UTF-8-sorted repository-relative path + NUL + kind + NUL + bytes + NUL + content SHA-256 + LF records',
    files: entries.length,
    bytes: entries.reduce((total, entry) => total + entry.bytes, 0),
    aggregateSha256: aggregate.digest('hex'),
    entries,
  };
}

async function captureFile(path, entries) {
  const fileStat = await lstat(path);
  const kind = fileStat.isSymbolicLink() ? 'symlink' : 'file';
  const content = kind === 'symlink' ? Buffer.from(await readlink(path)) : await readFile(path);
  entries.push({
    path: nodePath.relative(repositoryRoot, path).split(nodePath.sep).join('/'),
    kind,
    bytes: content.byteLength,
    sha256: sha256(content),
  });
}

async function walk(directory, entries) {
  const directoryEntries = await readdir(directory, { withFileTypes: true });
  directoryEntries.sort((left, right) => compareUtf8(left.name, right.name));
  for (const entry of directoryEntries) {
    if (entry.isDirectory() && ignoredDirectories.has(entry.name)) continue;
    const path = nodePath.join(directory, entry.name);
    if (entry.isDirectory()) {
      await walk(path, entries);
      continue;
    }
    await captureFile(path, entries);
  }
}

export function hashBytes(value) {
  return sha256(value);
}
