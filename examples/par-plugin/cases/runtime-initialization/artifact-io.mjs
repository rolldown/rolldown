import { randomUUID } from 'node:crypto';
import { mkdir, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';

export async function assertDistinctArtifactPaths(inputPath, outputPath) {
  if (!outputPath) return;
  const input = await canonicalArtifactPath(inputPath, false);
  const output = await canonicalArtifactPath(outputPath, true);
  if (input === output) {
    throw new Error('initialization summary output must not overwrite its raw input');
  }
}

export async function assertArtifactUnchanged(path, expectedBytes) {
  const current = await readFile(path);
  if (!current.equals(expectedBytes)) {
    throw new Error('initialization raw artifact changed while its summary was created');
  }
}

export async function writeArtifactAtomically(path, content) {
  const outputPath = nodePath.resolve(path);
  const directory = nodePath.dirname(outputPath);
  await mkdir(directory, { recursive: true });
  const temporaryPath = nodePath.join(
    directory,
    `.${nodePath.basename(outputPath)}.${process.pid}.${randomUUID()}.tmp`,
  );
  try {
    await writeFile(temporaryPath, content, { flag: 'wx' });
    await rename(temporaryPath, outputPath);
  } finally {
    await rm(temporaryPath, { force: true });
  }
}

async function canonicalArtifactPath(path, createParent) {
  const resolved = nodePath.resolve(path);
  try {
    return await realpath(resolved);
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
    const parent = nodePath.dirname(resolved);
    if (createParent) await mkdir(parent, { recursive: true });
    return nodePath.join(await realpath(parent), nodePath.basename(resolved));
  }
}
