import { createHash } from 'node:crypto';
import { copyFile, mkdir, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';

export const UPSTREAM_REPOSITORY = 'https://github.com/huntabyte/shadcn-svelte.git';
export const UPSTREAM_COMMIT = 'efcf8a4ef2c6a3a21ee2fd4db905519f8d4c8e63';
export const UPSTREAM_DIRECTORY = 'docs/src/lib/registry';
export const UPSTREAM_LICENSE_SHA256 =
  '230594c9dedb06fb0759381a520b449c1b47b0f63b95fc88ef9d7e851e2540a9';
export const EXPECTED_CORPUS = {
  files: 1340,
  lines: 64392,
  bytes: 1946145,
  typeScriptFiles: 1314,
  runeFiles: 616,
  uniqueContents: 1322,
  aggregateSha256: 'ea584b2189062d5986cb4c15f344bcb42cbee8b7089277ee95d5d7ab9f49b8e8',
};

const toPosixPath = (value) => value.split(nodePath.sep).join('/');
const compareUtf8 = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const sha256 = (value) => createHash('sha256').update(value).digest('hex');

async function walk(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const paths = await Promise.all(
    entries.map((entry) => {
      const entryPath = nodePath.join(directory, entry.name);
      return entry.isDirectory() ? walk(entryPath) : entryPath;
    }),
  );
  return paths.flat();
}

export async function scanSourceCorpus(sourceRoot) {
  const sourceDirectory = nodePath.join(sourceRoot, UPSTREAM_DIRECTORY);
  const allSveltePaths = (await walk(sourceDirectory)).filter((path) => path.endsWith('.svelte'));
  const entries = [];
  let excludedSvgFiles = 0;

  for (const absolutePath of allSveltePaths) {
    const content = await readFile(absolutePath);
    const source = content.toString('utf8');
    // ASCII case-insensitive matching intentionally also excludes the one
    // `SVGAttributes<SVGSVGElement>` occurrence, reproducing the pinned set.
    if (source.toLowerCase().includes('<svg')) {
      excludedSvgFiles++;
      continue;
    }
    entries.push({
      path: toPosixPath(nodePath.relative(sourceRoot, absolutePath)),
      bytes: content.byteLength,
      lines: source.match(/\n/g)?.length ?? 0,
      sha256: sha256(content),
      typeScript: /<script(?:\s[^>]*)?\slang=["']ts["']/.test(source),
      runes: /\$(?:state|derived|effect|props|bindable|inspect|host)\b/.test(source),
    });
  }

  entries.sort((left, right) => compareUtf8(left.path, right.path));
  return {
    entries,
    excludedSvgFiles,
    summary: summarizeEntries(entries),
  };
}

export function summarizeEntries(entries) {
  const aggregate = createHash('sha256');
  for (const entry of entries) {
    aggregate.update(entry.path);
    aggregate.update('\0');
    aggregate.update(String(entry.bytes));
    aggregate.update('\0');
    aggregate.update(entry.sha256);
    aggregate.update('\n');
  }
  return {
    files: entries.length,
    lines: entries.reduce((total, entry) => total + entry.lines, 0),
    bytes: entries.reduce((total, entry) => total + entry.bytes, 0),
    typeScriptFiles: entries.filter((entry) => entry.typeScript).length,
    runeFiles: entries.filter((entry) => entry.runes).length,
    uniqueContents: new Set(entries.map((entry) => entry.sha256)).size,
    aggregateSha256: aggregate.digest('hex'),
  };
}

export function assertExpectedCorpus(scan) {
  if (scan.excludedSvgFiles !== 26) {
    throw new Error(`expected 26 excluded SVG matches, got ${scan.excludedSvgFiles}`);
  }
  for (const [name, expected] of Object.entries(EXPECTED_CORPUS)) {
    if (scan.summary[name] !== expected) {
      throw new Error(`corpus ${name} mismatch: ${scan.summary[name]} != ${expected}`);
    }
  }
}

export async function readCorpusManifest(manifestPath) {
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  if (
    manifest.schema !== 1 ||
    manifest.upstream.repository !== UPSTREAM_REPOSITORY ||
    manifest.upstream.commit !== UPSTREAM_COMMIT ||
    manifest.summary.aggregateSha256 !== EXPECTED_CORPUS.aggregateSha256
  ) {
    throw new Error('unsupported or unexpected Svelte corpus manifest');
  }
  assertExpectedCorpus({
    excludedSvgFiles: manifest.selection.excludedSvgFiles,
    summary: summarizeEntries(manifest.entries),
  });
  return manifest;
}

export function createCorpusManifest(scan) {
  assertExpectedCorpus(scan);
  return {
    schema: 1,
    upstream: {
      repository: UPSTREAM_REPOSITORY,
      commit: UPSTREAM_COMMIT,
      directory: UPSTREAM_DIRECTORY,
      license: 'MIT',
      licenseSha256: UPSTREAM_LICENSE_SHA256,
    },
    selection: {
      glob: 'docs/src/lib/registry/**/*.svelte',
      exclusion: 'source.toLowerCase().includes("<svg")',
      excludedSvgFiles: scan.excludedSvgFiles,
      ordering: `sha256("${scan.summary.aggregateSha256}\\0" + path), then UTF-8 path`,
    },
    summary: scan.summary,
    entries: scan.entries,
  };
}

export function selectManifestEntries(manifest, count) {
  if (!Number.isSafeInteger(count) || count < 1 || count > manifest.entries.length) {
    throw new Error(`component count must be between 1 and ${manifest.entries.length}`);
  }
  return [...manifest.entries]
    .map((entry) => ({
      entry,
      orderHash: sha256(`${manifest.summary.aggregateSha256}\0${entry.path}`),
    }))
    .sort(
      (left, right) =>
        compareUtf8(left.orderHash, right.orderHash) ||
        compareUtf8(left.entry.path, right.entry.path),
    )
    .slice(0, count)
    .map(({ entry }) => entry);
}

export function selectionHash(entries) {
  const hash = createHash('sha256');
  for (const entry of entries) {
    hash.update(entry.path);
    hash.update('\0');
    hash.update(entry.sha256);
    hash.update('\n');
  }
  return hash.digest('hex');
}

export async function prepareCorpus({ sourceRoot, destination, manifestPath, updateManifest }) {
  const scan = await scanSourceCorpus(sourceRoot);
  assertExpectedCorpus(scan);
  const sourceLicense = await readFile(nodePath.join(sourceRoot, 'LICENSE.md'));
  if (sha256(sourceLicense) !== UPSTREAM_LICENSE_SHA256) {
    throw new Error('upstream license hash mismatch');
  }

  if (updateManifest) {
    await writeFile(manifestPath, `${JSON.stringify(createCorpusManifest(scan), null, 2)}\n`);
  }
  const manifest = await readCorpusManifest(manifestPath);
  if (JSON.stringify(scan.entries) !== JSON.stringify(manifest.entries)) {
    throw new Error('source checkout does not match the committed corpus manifest');
  }

  const temporaryDestination = `${destination}.tmp-${process.pid}`;
  await rm(temporaryDestination, { recursive: true, force: true });
  try {
    for (const entry of manifest.entries) {
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
          aggregateSha256: manifest.summary.aggregateSha256,
          files: manifest.summary.files,
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

export async function verifyPreparedCorpus({ corpusDirectory, manifest }) {
  const snapshot = JSON.parse(
    await readFile(nodePath.join(corpusDirectory, '.snapshot.json'), 'utf8'),
  );
  if (
    snapshot.upstreamCommit !== UPSTREAM_COMMIT ||
    snapshot.aggregateSha256 !== manifest.summary.aggregateSha256 ||
    snapshot.files !== manifest.summary.files
  ) {
    throw new Error('prepared Svelte corpus snapshot metadata mismatch');
  }
  for (const entry of manifest.entries) {
    const content = await readFile(nodePath.join(corpusDirectory, entry.path));
    if (content.byteLength !== entry.bytes || sha256(content) !== entry.sha256) {
      throw new Error(`prepared Svelte corpus mismatch: ${entry.path}`);
    }
  }
}
