import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, readdir } from 'node:fs/promises';
import nodePath from 'node:path';

export const UPSTREAM_URL = 'https://github.com/cabinet-fe/icon.git';
export const UPSTREAM_COMMIT = '9cadad32c72d79424c75e3b6e56798f216bb0b06';
export const EXPECTED_SFC_COUNT = 166;
export const EXPECTED_SFC_BYTES = 109122;
export const EXPECTED_MANIFEST_HASH =
  '9ae54c3311168ccd093c9da5a1e977c81654590ce040a5de63c2702ff0f3fedd';
export const EXPECTED_COLORFUL_SFC_COUNT = 12;
export const EXPECTED_COLORFUL_SFC_BYTES = 16932;
export const EXPECTED_COLORFUL_MANIFEST_HASH =
  '6b8c33346f17113a20a245c684cc38f8c9549db519a9d27809376b505ea4c083';

const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
export const upstreamDirectory = nodePath.join(repositoryRoot, 'tmp/bench/vue-icon-upstream');
export const corpusDirectory = nodePath.join(upstreamDirectory, 'packages/vue');
export const sfcRoot = nodePath.join(corpusDirectory, 'src');
export const entryPaths = [
  'src/index.ts',
  'src/normal/index.ts',
  'src/colorful/index.ts',
  'src/names.ts',
];

const runGit = (args, cwd = repositoryRoot) => {
  const result = spawnSync('git', args, { cwd, encoding: 'utf8' });
  if (result.status !== 0) {
    throw new Error(`git ${args.join(' ')} failed:\n${result.stderr}`);
  }
  return result.stdout.trim();
};

const walk = async (directory) =>
  (
    await Promise.all(
      (
        await readdir(directory, { withFileTypes: true })
      ).map((entry) => {
        const path = nodePath.join(directory, entry.name);
        return entry.isDirectory() ? walk(path) : path;
      }),
    )
  ).flat();

export async function ensureVueCorpus() {
  await mkdir(nodePath.dirname(upstreamDirectory), { recursive: true });
  try {
    runGit(['-C', upstreamDirectory, 'rev-parse', '--git-dir']);
  } catch {
    runGit(['clone', '--filter=blob:none', '--no-checkout', UPSTREAM_URL, upstreamDirectory]);
  }

  try {
    runGit(['-C', upstreamDirectory, 'cat-file', '-e', `${UPSTREAM_COMMIT}^{commit}`]);
  } catch {
    runGit(['-C', upstreamDirectory, 'fetch', '--depth=1', 'origin', UPSTREAM_COMMIT]);
  }
  runGit(['-C', upstreamDirectory, 'checkout', '--detach', UPSTREAM_COMMIT]);

  const files = (await walk(sfcRoot))
    .filter((path) => path.endsWith('.vue'))
    .sort((left, right) =>
      Buffer.from(nodePath.relative(sfcRoot, left)).compare(
        Buffer.from(nodePath.relative(sfcRoot, right)),
      ),
    );
  let totalBytes = 0;
  let manifest = '';
  for (const path of files) {
    const content = await readFile(path);
    const relativePath = nodePath.relative(sfcRoot, path);
    totalBytes += content.length;
    manifest += `${relativePath}\0${content.length}\0${createHash('sha256').update(content).digest('hex')}\n`;
  }
  const manifestHash = createHash('sha256').update(manifest).digest('hex');
  if (
    files.length !== EXPECTED_SFC_COUNT ||
    totalBytes !== EXPECTED_SFC_BYTES ||
    manifestHash !== EXPECTED_MANIFEST_HASH
  ) {
    throw new Error(
      `Vue corpus mismatch: ${JSON.stringify({ count: files.length, totalBytes, manifestHash })}`,
    );
  }
  const colorfulFiles = files.filter(
    (path) => nodePath.dirname(path) === nodePath.join(sfcRoot, 'colorful'),
  );
  let colorfulBytes = 0;
  let colorfulManifest = '';
  for (const path of colorfulFiles) {
    const content = await readFile(path);
    const relativePath = nodePath.relative(nodePath.join(sfcRoot, 'colorful'), path);
    colorfulBytes += content.length;
    colorfulManifest += `${relativePath}\0${content.length}\0${createHash('sha256').update(content).digest('hex')}\n`;
  }
  const colorfulManifestHash = createHash('sha256').update(colorfulManifest).digest('hex');
  if (
    colorfulFiles.length !== EXPECTED_COLORFUL_SFC_COUNT ||
    colorfulBytes !== EXPECTED_COLORFUL_SFC_BYTES ||
    colorfulManifestHash !== EXPECTED_COLORFUL_MANIFEST_HASH
  ) {
    throw new Error(
      `Vue colorful corpus mismatch: ${JSON.stringify({ count: colorfulFiles.length, colorfulBytes, colorfulManifestHash })}`,
    );
  }
  for (const entry of entryPaths) await readFile(nodePath.join(corpusDirectory, entry));

  return {
    upstreamUrl: UPSTREAM_URL,
    upstreamCommit: UPSTREAM_COMMIT,
    corpusDirectory,
    sfcRoot,
    entryPaths,
    sfcCount: files.length,
    totalSfcBytes: totalBytes,
    manifestHash,
    corpora: {
      full: {
        entryPaths,
        sfcCount: files.length,
        totalSfcBytes: totalBytes,
        manifestHash,
      },
      colorful: {
        entryPaths: ['src/colorful/index.ts'],
        sfcCount: colorfulFiles.length,
        totalSfcBytes: colorfulBytes,
        manifestHash: colorfulManifestHash,
      },
    },
  };
}

if (process.argv[1] === import.meta.filename) {
  console.log(JSON.stringify(await ensureVueCorpus(), null, 2));
}
