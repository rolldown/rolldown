import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { copyFile, mkdir, readFile, rename, rm, stat, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';

export const UPSTREAM_REPOSITORY = 'https://github.com/huntabyte/shadcn-svelte.git';
export const UPSTREAM_COMMIT = 'efcf8a4ef2c6a3a21ee2fd4db905519f8d4c8e63';
export const UPSTREAM_LICENSE_SHA256 =
  '230594c9dedb06fb0759381a520b449c1b47b0f63b95fc88ef9d7e851e2540a9';
export const SOURCE_DIRECTORY = 'docs/src';
export const EXPECTED_SOURCE = {
  files: 2607,
  bytes: 3535740,
  svelteFiles: 1658,
  typeScriptFiles: 938,
  cssFiles: 9,
  markdownFiles: 1,
  htmlFiles: 1,
  aggregateSha256: 'd7e6608eee8465062fae46ab0343837cdcee39838fadb0106ae24755030c3e4c',
  entryCount: 56,
  entryAggregateSha256: '94c211bee6a329c7ceceea46c6d9a6a48d404b23519f931886b2c2891b3927d8',
};

const compareUtf8 = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const sha256 = (value) => createHash('sha256').update(value).digest('hex');

export async function scanSourceCheckout(sourceRoot) {
  const tracked = spawnSync('git', ['-C', sourceRoot, 'ls-files', '-z', SOURCE_DIRECTORY], {
    encoding: 'buffer',
  });
  if (tracked.status !== 0) throw new Error('failed to list pinned source files');
  const paths = tracked.stdout.toString().split('\0').filter(Boolean).sort(compareUtf8);
  const entries = [];
  for (const path of paths) {
    const content = await readFile(nodePath.join(sourceRoot, path));
    entries.push({ path, bytes: content.byteLength, sha256: sha256(content) });
  }
  const entryPaths = paths.filter((path) =>
    /^docs\/src\/lib\/registry\/ui\/[^/]+\/index\.ts$/.test(path),
  );
  const aggregate = createHash('sha256');
  for (const entry of entries) {
    aggregate.update(entry.path);
    aggregate.update('\0');
    aggregate.update(String(entry.bytes));
    aggregate.update('\0');
    aggregate.update(entry.sha256);
    aggregate.update('\n');
  }
  const entryAggregate = createHash('sha256');
  for (const path of entryPaths) {
    const entry = entries.find((candidate) => candidate.path === path);
    entryAggregate.update(path);
    entryAggregate.update('\0');
    entryAggregate.update(entry.sha256);
    entryAggregate.update('\n');
  }
  return {
    entries,
    entryPaths,
    summary: {
      files: entries.length,
      bytes: entries.reduce((total, entry) => total + entry.bytes, 0),
      svelteFiles: paths.filter((path) => path.endsWith('.svelte')).length,
      typeScriptFiles: paths.filter((path) => path.endsWith('.ts')).length,
      cssFiles: paths.filter((path) => path.endsWith('.css')).length,
      markdownFiles: paths.filter((path) => path.endsWith('.md')).length,
      htmlFiles: paths.filter((path) => path.endsWith('.html')).length,
      aggregateSha256: aggregate.digest('hex'),
      entryCount: entryPaths.length,
      entryAggregateSha256: entryAggregate.digest('hex'),
    },
  };
}

export function assertExpectedSource(scan) {
  for (const [name, expected] of Object.entries(EXPECTED_SOURCE)) {
    if (scan.summary[name] !== expected) {
      throw new Error(`graph source ${name} mismatch: ${scan.summary[name]} != ${expected}`);
    }
  }
}

export async function readSourceManifest(manifestPath) {
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  if (
    manifest.schema !== 1 ||
    manifest.upstream.repository !== UPSTREAM_REPOSITORY ||
    manifest.upstream.commit !== UPSTREAM_COMMIT
  ) {
    throw new Error('unexpected Svelte registry graph manifest');
  }
  assertExpectedSource({ summary: manifest.summary });
  if (manifest.entryPaths.length !== EXPECTED_SOURCE.entryCount) {
    throw new Error('registry graph entry count mismatch');
  }
  return manifest;
}

export async function prepareGraphCorpus({
  sourceRoot,
  destination,
  manifestPath,
  updateManifest,
}) {
  const head = spawnSync('git', ['-C', sourceRoot, 'rev-parse', 'HEAD'], { encoding: 'utf8' });
  if (head.status !== 0 || head.stdout.trim() !== UPSTREAM_COMMIT) {
    throw new Error(`source checkout must be exactly ${UPSTREAM_COMMIT}`);
  }
  const scan = await scanSourceCheckout(sourceRoot);
  assertExpectedSource(scan);
  const license = await readFile(nodePath.join(sourceRoot, 'LICENSE.md'));
  if (sha256(license) !== UPSTREAM_LICENSE_SHA256) throw new Error('upstream license mismatch');
  if (updateManifest) {
    await writeFile(
      manifestPath,
      `${JSON.stringify(
        {
          schema: 1,
          upstream: {
            repository: UPSTREAM_REPOSITORY,
            commit: UPSTREAM_COMMIT,
            license: 'MIT',
            licenseSha256: UPSTREAM_LICENSE_SHA256,
          },
          sourceDirectory: SOURCE_DIRECTORY,
          summary: scan.summary,
          entryPaths: scan.entryPaths,
        },
        null,
        2,
      )}\n`,
    );
  }
  const manifest = await readSourceManifest(manifestPath);
  if (JSON.stringify(manifest.entryPaths) !== JSON.stringify(scan.entryPaths)) {
    throw new Error('entry barrels differ from the committed manifest');
  }

  const temporaryDestination = `${destination}.tmp-${process.pid}`;
  await rm(temporaryDestination, { recursive: true, force: true });
  try {
    for (const entry of scan.entries) {
      const outputPath = nodePath.join(temporaryDestination, entry.path);
      await mkdir(nodePath.dirname(outputPath), { recursive: true });
      await copyFile(nodePath.join(sourceRoot, entry.path), outputPath);
    }
    await writeFile(
      nodePath.join(temporaryDestination, '.snapshot.json'),
      `${JSON.stringify(
        {
          schema: 1,
          upstreamCommit: UPSTREAM_COMMIT,
          summary: scan.summary,
          entries: scan.entries,
        },
        null,
        2,
      )}\n`,
    );
    await rm(destination, { recursive: true, force: true });
    await rename(temporaryDestination, destination);
  } finally {
    await rm(temporaryDestination, { recursive: true, force: true });
  }
  return manifest;
}

export async function verifyGraphCorpus({ corpusDirectory, manifest }) {
  const snapshot = JSON.parse(
    await readFile(nodePath.join(corpusDirectory, '.snapshot.json'), 'utf8'),
  );
  if (
    snapshot.upstreamCommit !== UPSTREAM_COMMIT ||
    snapshot.summary.aggregateSha256 !== manifest.summary.aggregateSha256 ||
    snapshot.entries.length !== manifest.summary.files
  ) {
    throw new Error('prepared registry graph snapshot metadata mismatch');
  }
  for (const entry of snapshot.entries) {
    const path = nodePath.join(corpusDirectory, entry.path);
    const fileStat = await stat(path);
    if (fileStat.size !== entry.bytes || sha256(await readFile(path)) !== entry.sha256) {
      throw new Error(`prepared registry graph source mismatch: ${entry.path}`);
    }
  }
  return snapshot;
}
