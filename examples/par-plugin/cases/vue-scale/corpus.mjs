import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  copyFile,
  lstat,
  mkdir,
  readFile,
  readlink,
  readdir,
  rename,
  rm,
  symlink,
  writeFile,
} from 'node:fs/promises';
import nodePath from 'node:path';
import { parse, version as compilerVersion } from '@vue/compiler-sfc';

export const VUE_COMPILER_VERSION = '3.5.39';
export const CORPUS_AGGREGATE_SHA256 =
  '114f8b7b7e3fa7d13d5f14946acd7a4a42d88957f7ca57da041381cd6eada99c';

export const REPOSITORIES = [
  {
    id: 'primevue',
    repository: 'https://github.com/primefaces/primevue.git',
    commit: 'd4374cb7c1267f35eba7cee5d0a266f50ca8ec84',
    license: 'MIT',
    licensePath: 'LICENSE.md',
    licenseSha256: '39a2ce8d759cfcb59eccc49b0a417ad5c943f960c1bcdfba4720ca7547029af7',
    eligibleSfcCount: 2495,
    eligibleSfcBytes: 8511875,
  },
  {
    id: 'element-plus',
    repository: 'https://github.com/element-plus/element-plus.git',
    commit: '85bdf740c1d550f3ca44472262e2a314039eab7d',
    license: 'MIT',
    licensePath: 'LICENSE',
    licenseSha256: '0790118bb4d66681db1d63181f72ef68e632d632f6db0373ef87cf328561af27',
    eligibleSfcCount: 725,
    eligibleSfcBytes: 1942309,
  },
  {
    id: 'tdesign-vue-next',
    repository: 'https://github.com/Tencent/tdesign-vue-next.git',
    commit: 'dd334e2dc06d8ab48d1b6ebc5e9d4f6de67b16a2',
    license: 'MIT',
    licensePath: 'LICENSE',
    licenseSha256: 'b3dbcb89dcf4a11abf1b70d043795a3da0c458af16fefd2ff315d9ff5875312f',
    eligibleSfcCount: 644,
    eligibleSfcBytes: 897120,
  },
  {
    id: 'vuestic-ui',
    repository: 'https://github.com/epicmaxco/vuestic-ui.git',
    commit: 'c5337ed8e7e24ea294221326fe2ca6af8d3b8e1b',
    license: 'MIT',
    licensePath: 'LICENSE.MD',
    licenseSha256: 'c44258bd026d8749142ac1b2cf0309f0b52655b3181c5ee4bfb6bd89103ab370',
    eligibleSfcCount: 676,
    eligibleSfcBytes: 882094,
  },
  {
    id: 'quasar',
    repository: 'https://github.com/quasarframework/quasar.git',
    commit: '2165ce9f69d84e6169e7ca8a1c51fde105042cb9',
    license: 'MIT',
    licensePath: 'LICENSE',
    licenseSha256: '830424149e83c3b9caa4243c36e73ac1b024b501fea99f8a22138b86eedc8d47',
    eligibleSfcCount: 1110,
    eligibleSfcBytes: 2244448,
    compileAdmissionExclusions: [
      'app-vite/playground-ts/src/components/EssentialLink.vue',
      'app-vite/playground-ts/src/pages/index/(index).vue',
      'app-vite/playground-ts/src/pages/index/second.vue',
    ],
  },
];

export const EXPECTED_CORPUS = {
  files: 5650,
  bytes: 14477846,
  scriptSetupFiles: 2505,
  ordinaryScriptFiles: 2247,
  templateOnlyFiles: 898,
  uniqueContents: 5650,
  aggregateSha256: CORPUS_AGGREGATE_SHA256,
};

export const FROZEN_SELECTIONS = {
  32: '542c27dc121c69009a27ebb77a75e2a5b8660b4e2c85ad3949c766af8ca59998',
  128: '1a1833a66bd645d2f63886493dc0749ad05de6728549b6bb8af62a1fc7ff3591',
  256: '0609df9cb9e6153bbd5a19325a7c82d17b4ec52f35c509f99bff94e67411100a',
  512: '6770cadb2c52ae19ad3776e969d204b9f458be1e26f8e6d28d4a463001274d93',
  1024: '5d01c401de0e559934961478783dcb36ca9ddaa98fc6bc987a62e81726fe7b34',
  2048: '2483b221836c7f86610095ddab18f9f7ca42e22d857558347f8f8f3cffbcfed9',
  4096: 'ffdfac9f785e570f8db341ce2afc1e66c40db8008e19c953e9bfc41e5829645f',
  5000: '27add878d7150bf40b5efc3540f0e78e029a6d4b076aae8914ba2b2ca7d6e474',
};

export const GENERATED_SUPPORT_SUMMARY = {
  files: 15,
  bytes: 28403,
  symlinks: 2,
  gitlinks: 0,
  aggregateSha256: '64370492a4d453788b0b6ef0134218814e192fcefe6d1dd4bc3f7264f3457c48',
};

const GENERATED_SUPPORT_REPOSITORY_SUMMARIES = {
  primevue: {
    files: 1,
    bytes: 7837,
    symlinks: 0,
    gitlinks: 0,
    aggregateSha256: '9234e25967dd4a8eda1f794945cd9d9b8766e20b7d4a414339fd3947f6b35c50',
  },
  'element-plus': {
    files: 2,
    bytes: 45,
    symlinks: 2,
    gitlinks: 0,
    aggregateSha256: 'f35a679cf3d13740931965693c6d3a7733d252360f45a72e6794a033b4fcee0c',
  },
  'tdesign-vue-next': {
    files: 0,
    bytes: 0,
    symlinks: 0,
    gitlinks: 0,
    aggregateSha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
  },
  'vuestic-ui': {
    files: 12,
    bytes: 20521,
    symlinks: 0,
    gitlinks: 0,
    aggregateSha256: 'ef01282754f5770eb332072f208d58c46c5274a04cef0a2b97ee56f8e1a9f04b',
  },
  quasar: {
    files: 0,
    bytes: 0,
    symlinks: 0,
    gitlinks: 0,
    aggregateSha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
  },
};

const GENERATED_SUPPORT_DIRECTORY = nodePath.join(import.meta.dirname, 'support-overlays');
const GENERATED_SUPPORT_SYMLINKS = [
  {
    repository: 'element-plus',
    path: 'node_modules/@element-plus/components',
    target: '../../packages/components',
  },
  {
    repository: 'element-plus',
    path: 'node_modules/@element-plus/hooks',
    target: '../../packages/hooks',
  },
];

const compareUtf8 = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const toPosixPath = (value) => value.split(nodePath.sep).join('/');

const trackedFiles = (sourceRoot, patterns = []) => {
  const result = spawnSync('git', ['-C', sourceRoot, 'ls-files', '-z', '--', ...patterns], {
    encoding: 'buffer',
  });
  if (result.status !== 0) {
    throw new Error(`failed to list tracked Vue sources in ${sourceRoot}: ${result.stderr}`);
  }
  return result.stdout.toString('utf8').split('\0').filter(Boolean).sort(compareUtf8);
};

const summarizeSupportEntries = (entries) => {
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
    files: entries.length,
    bytes: entries.reduce((total, entry) => total + entry.bytes, 0),
    symlinks: entries.filter((entry) => entry.kind === 'symlink').length,
    gitlinks: entries.filter((entry) => entry.kind === 'gitlink').length,
    aggregateSha256: aggregate.digest('hex'),
  };
};

const scanGeneratedSupportTree = async () => {
  const entriesByRepository = Object.fromEntries(REPOSITORIES.map(({ id }) => [id, []]));
  async function walk(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const path = nodePath.join(directory, entry.name);
      if (entry.isDirectory()) {
        await walk(path);
        continue;
      }
      if (!entry.isFile()) throw new Error(`unsupported committed support overlay: ${path}`);
      const relativePath = toPosixPath(nodePath.relative(GENERATED_SUPPORT_DIRECTORY, path));
      const [repository, ...segments] = relativePath.split('/');
      if (!Object.hasOwn(entriesByRepository, repository) || segments.length === 0) {
        throw new Error(`invalid committed support overlay path: ${relativePath}`);
      }
      const content = await readFile(path);
      entriesByRepository[repository].push({
        path: segments.join('/'),
        kind: 'file',
        bytes: content.byteLength,
        sha256: sha256(content),
      });
    }
  }
  await walk(GENERATED_SUPPORT_DIRECTORY);
  for (const entry of GENERATED_SUPPORT_SYMLINKS) {
    const content = Buffer.from(entry.target);
    entriesByRepository[entry.repository].push({
      path: entry.path,
      kind: 'symlink',
      target: entry.target,
      bytes: content.byteLength,
      sha256: sha256(content),
    });
  }
  for (const entries of Object.values(entriesByRepository)) {
    entries.sort((left, right) => compareUtf8(left.path, right.path));
  }
  const summaries = Object.fromEntries(
    REPOSITORIES.map(({ id }) => [id, summarizeSupportEntries(entriesByRepository[id])]),
  );
  if (JSON.stringify(summaries) !== JSON.stringify(GENERATED_SUPPORT_REPOSITORY_SUMMARIES)) {
    throw new Error('committed generated Vue support overlay summary mismatch');
  }
  const aggregateEntries = REPOSITORIES.flatMap(({ id }) =>
    entriesByRepository[id].map((entry) => ({ ...entry, path: `${id}/${entry.path}` })),
  ).sort((left, right) => compareUtf8(left.path, right.path));
  if (
    JSON.stringify(summarizeSupportEntries(aggregateEntries)) !==
    JSON.stringify(GENERATED_SUPPORT_SUMMARY)
  ) {
    throw new Error('committed generated Vue support overlay aggregate mismatch');
  }
  return { entriesByRepository, summaries };
};

const scanSupportTree = async (sourceRoot) => {
  const entries = [];
  for (const relativePath of trackedFiles(sourceRoot)) {
    const absolutePath = nodePath.join(sourceRoot, relativePath);
    const fileStat = await lstat(absolutePath);
    let kind;
    let content;
    if (fileStat.isSymbolicLink()) {
      kind = 'symlink';
      content = Buffer.from(await readlink(absolutePath));
    } else if (fileStat.isFile()) {
      kind = 'file';
      content = await readFile(absolutePath);
    } else if (fileStat.isDirectory()) {
      kind = 'gitlink';
      const stage = spawnSync(
        'git',
        ['-C', sourceRoot, 'ls-files', '--stage', '--', relativePath],
        { encoding: 'utf8' },
      );
      const match = stage.stdout.match(/^160000 ([a-f0-9]{40}) 0\t/);
      if (stage.status !== 0 || !match) {
        throw new Error(`unsupported tracked Vue support directory: ${absolutePath}`);
      }
      content = Buffer.from(match[1]);
    } else {
      throw new Error(`unsupported tracked Vue support entry: ${absolutePath}`);
    }
    const entry = {
      path: toPosixPath(relativePath),
      kind,
      bytes: content.byteLength,
      sha256: sha256(content),
    };
    if (kind !== 'file') entry.target = content.toString('utf8');
    entries.push(entry);
  }
  return { entries, summary: summarizeSupportEntries(entries) };
};

const checkoutHead = (sourceRoot) => {
  const result = spawnSync('git', ['-C', sourceRoot, 'rev-parse', 'HEAD'], {
    encoding: 'utf8',
  });
  if (result.status !== 0) throw new Error(`failed to identify checkout at ${sourceRoot}`);
  return result.stdout.trim();
};

const checkoutStatus = (sourceRoot) => {
  const result = spawnSync('git', ['-C', sourceRoot, 'status', '--short'], {
    encoding: 'utf8',
  });
  if (result.status !== 0) throw new Error(`failed to inspect checkout at ${sourceRoot}`);
  return result.stdout.trim();
};

const assertNoCheckoutNodeModules = async (sourceRoot) => {
  try {
    await lstat(nodePath.join(sourceRoot, 'node_modules'));
  } catch (error) {
    if (error?.code === 'ENOENT') return;
    throw error;
  }
  throw new Error(`pinned Vue support checkout must not contain node_modules: ${sourceRoot}`);
};

const contentKind = (descriptor) => {
  if (descriptor.scriptSetup) return 'script-setup';
  if (descriptor.script) return 'ordinary-script';
  return 'template-only';
};

const hasExternalBlockSource = (descriptor) =>
  [
    descriptor.template,
    descriptor.script,
    descriptor.scriptSetup,
    ...descriptor.styles,
    ...descriptor.customBlocks,
  ]
    .filter(Boolean)
    .some((block) => block.src);

export function classifyVueSource(content, filename) {
  const parsed = parse(Buffer.isBuffer(content) ? content.toString('utf8') : content, { filename });
  if (parsed.errors.length !== 0) return { eligible: false, reason: 'parse' };
  const { descriptor } = parsed;
  if (descriptor.styles.length !== 0 || descriptor.customBlocks.length !== 0) {
    return { eligible: false, reason: 'blocks' };
  }
  if (hasExternalBlockSource(descriptor)) {
    return { eligible: false, reason: 'external-source' };
  }
  if (descriptor.template?.lang && descriptor.template.lang !== 'html') {
    return { eligible: false, reason: 'template-language' };
  }
  return { eligible: true, kind: contentKind(descriptor) };
}

export async function scanSourceCorpora(sourceRoots) {
  if (compilerVersion !== VUE_COMPILER_VERSION) {
    throw new Error(
      `Vue corpus requires @vue/compiler-sfc ${VUE_COMPILER_VERSION}, got ${compilerVersion}`,
    );
  }

  const candidates = [];
  const repositoryScans = [];
  const supportEntriesByRepository = {};
  for (const repository of REPOSITORIES) {
    const sourceRoot = sourceRoots[repository.id];
    if (typeof sourceRoot !== 'string' || sourceRoot.length === 0) {
      throw new Error(`missing source checkout for ${repository.id}`);
    }
    if (checkoutHead(sourceRoot) !== repository.commit) {
      throw new Error(`${repository.id} checkout must be exactly ${repository.commit}`);
    }
    if (checkoutStatus(sourceRoot) !== '') {
      throw new Error(`${repository.id} checkout must be clean`);
    }
    await assertNoCheckoutNodeModules(sourceRoot);
    const license = await readFile(nodePath.join(sourceRoot, repository.licensePath));
    if (sha256(license) !== repository.licenseSha256) {
      throw new Error(`${repository.id} license hash mismatch`);
    }

    const support = await scanSupportTree(sourceRoot);
    supportEntriesByRepository[repository.id] = support.entries;
    const vueFiles = trackedFiles(sourceRoot, ['*.vue']);
    let parseFailures = 0;
    let blockExclusions = 0;
    let externalSourceExclusions = 0;
    let nonPlainTemplateExclusions = 0;
    let compileAdmissionPathExclusions = 0;
    for (const relativePath of vueFiles) {
      const content = await readFile(nodePath.join(sourceRoot, relativePath));
      const classification = classifyVueSource(content, relativePath);
      if (!classification.eligible && classification.reason === 'parse') {
        parseFailures++;
        continue;
      }
      if (!classification.eligible && classification.reason === 'blocks') {
        blockExclusions++;
        continue;
      }
      if (!classification.eligible && classification.reason === 'external-source') {
        externalSourceExclusions++;
        continue;
      }
      if (!classification.eligible && classification.reason === 'template-language') {
        nonPlainTemplateExclusions++;
        continue;
      }
      if (!classification.eligible) throw new Error('unhandled Vue source eligibility result');
      if (repository.compileAdmissionExclusions?.includes(toPosixPath(relativePath))) {
        compileAdmissionPathExclusions++;
        continue;
      }
      candidates.push({
        repository: repository.id,
        path: toPosixPath(relativePath),
        sourceKey: `${repository.id}/${toPosixPath(relativePath)}`,
        bytes: content.byteLength,
        sha256: sha256(content),
        kind: classification.kind,
      });
    }
    repositoryScans.push({
      id: repository.id,
      trackedSfcCount: vueFiles.length,
      parseFailures,
      blockExclusions,
      externalSourceExclusions,
      nonPlainTemplateExclusions,
      compileAdmissionPathExclusions,
      support: support.summary,
    });
  }

  candidates.sort((left, right) => compareUtf8(left.sourceKey, right.sourceKey));
  const retainedContentHashes = new Set();
  const entries = [];
  for (const candidate of candidates) {
    if (retainedContentHashes.has(candidate.sha256)) continue;
    retainedContentHashes.add(candidate.sha256);
    entries.push(candidate);
  }
  const summary = summarizeEntries(entries);
  assertExpectedCorpus(summary);

  const repositories = REPOSITORIES.map((repository) => {
    const entriesForRepository = entries.filter((entry) => entry.repository === repository.id);
    const actual = {
      eligibleSfcCount: entriesForRepository.length,
      eligibleSfcBytes: entriesForRepository.reduce((total, entry) => total + entry.bytes, 0),
    };
    if (
      actual.eligibleSfcCount !== repository.eligibleSfcCount ||
      actual.eligibleSfcBytes !== repository.eligibleSfcBytes
    ) {
      throw new Error(`${repository.id} retained corpus mismatch: ${JSON.stringify(actual)}`);
    }
    return {
      ...repository,
      ...repositoryScans.find((scan) => scan.id === repository.id),
    };
  });
  return { entries, repositories, summary, supportEntriesByRepository };
}

export function summarizeEntries(entries) {
  const aggregate = createHash('sha256');
  for (const entry of [...entries].sort((left, right) =>
    compareUtf8(left.sourceKey, right.sourceKey),
  )) {
    aggregate.update(entry.sourceKey);
    aggregate.update('\0');
    aggregate.update(String(entry.bytes));
    aggregate.update('\0');
    aggregate.update(entry.sha256);
    aggregate.update('\n');
  }
  return {
    files: entries.length,
    bytes: entries.reduce((total, entry) => total + entry.bytes, 0),
    scriptSetupFiles: entries.filter((entry) => entry.kind === 'script-setup').length,
    ordinaryScriptFiles: entries.filter((entry) => entry.kind === 'ordinary-script').length,
    templateOnlyFiles: entries.filter((entry) => entry.kind === 'template-only').length,
    uniqueContents: new Set(entries.map((entry) => entry.sha256)).size,
    aggregateSha256: aggregate.digest('hex'),
  };
}

export function assertExpectedCorpus(summary) {
  for (const [name, expected] of Object.entries(EXPECTED_CORPUS)) {
    if (summary[name] !== expected) {
      throw new Error(`Vue scale corpus ${name} mismatch: ${summary[name]} != ${expected}`);
    }
  }
}

export function selectManifestEntries(manifest, count) {
  if (!Object.hasOwn(FROZEN_SELECTIONS, count)) {
    throw new Error(`component count is not a frozen Vue scale: ${count}`);
  }
  return [...manifest.entries]
    .map((entry) => ({
      entry,
      orderHash: sha256(`${manifest.summary.aggregateSha256}\0${entry.sourceKey}`),
    }))
    .sort(
      (left, right) =>
        compareUtf8(left.orderHash, right.orderHash) ||
        compareUtf8(left.entry.sourceKey, right.entry.sourceKey),
    )
    .slice(0, count)
    .map(({ entry }) => entry);
}

export function selectionHash(entries) {
  const hash = createHash('sha256');
  for (const entry of entries) {
    hash.update(entry.sourceKey);
    hash.update('\0');
    hash.update(entry.sha256);
    hash.update('\n');
  }
  return hash.digest('hex');
}

export function summarizeSelection(entries) {
  return {
    files: entries.length,
    bytes: entries.reduce((total, entry) => total + entry.bytes, 0),
    scriptSetupFiles: entries.filter((entry) => entry.kind === 'script-setup').length,
    ordinaryScriptFiles: entries.filter((entry) => entry.kind === 'ordinary-script').length,
    templateOnlyFiles: entries.filter((entry) => entry.kind === 'template-only').length,
    repositories: Object.fromEntries(
      REPOSITORIES.map(({ id }) => [id, entries.filter((entry) => entry.repository === id).length]),
    ),
    selectionSha256: selectionHash(entries),
  };
}

export async function summarizeSelectionInput(entries, corpusDirectory) {
  const input = createHash('sha256');
  const exactOnce = createHash('sha256');
  let bytes = 0;
  for (const entry of entries) {
    const content = await readFile(nodePath.join(corpusDirectory, entry.sourceKey));
    const contentSha256 = sha256(content);
    if (content.byteLength !== entry.bytes || contentSha256 !== entry.sha256) {
      throw new Error(`selected Vue input differs from the manifest: ${entry.sourceKey}`);
    }
    bytes += content.byteLength;
    input.update(entry.sourceKey);
    input.update('\0');
    input.update(String(content.byteLength));
    input.update('\0');
    input.update(contentSha256);
    input.update('\n');
    exactOnce.update(entry.sourceKey);
    exactOnce.update('\0');
    exactOnce.update('1');
    exactOnce.update('\n');
  }
  return {
    files: entries.length,
    bytes,
    aggregateSha256: input.digest('hex'),
    exactOnceSha256: exactOnce.digest('hex'),
  };
}

export function createCorpusManifest(scan) {
  assertExpectedCorpus(scan.summary);
  const selections = Object.fromEntries(
    Object.keys(FROZEN_SELECTIONS).map((count) => {
      const entries = selectManifestEntries(
        { entries: scan.entries, summary: scan.summary },
        Number(count),
      );
      const summary = summarizeSelection(entries);
      if (summary.selectionSha256 !== FROZEN_SELECTIONS[count]) {
        throw new Error(`frozen Vue selection hash mismatch at ${count}`);
      }
      return [count, summary];
    }),
  );
  return {
    schema: 2,
    compiler: { package: '@vue/compiler-sfc', version: VUE_COMPILER_VERSION },
    repositories: scan.repositories,
    eligibility: {
      include: 'tracked **/*.vue',
      parse: 'parse succeeds with @vue/compiler-sfc 3.5.39',
      exclude: [
        'any style block',
        'any custom block',
        'template, script, script-setup, style, or custom block with src',
        'template language other than the default HTML compiler',
        'an exact source that fails the pinned ordinary unplugin-vue compile admission in its tracked and frozen generated support tree',
      ],
      deduplicate: 'retain the first UTF-8-sorted sourceKey for each exact source SHA-256',
      aggregate:
        'SHA-256 over UTF-8-sorted sourceKey + NUL + byteLength + NUL + contentSha256 + LF records',
      nestedOrder: 'SHA-256(aggregateSha256 + NUL + sourceKey), then UTF-8 sourceKey',
      selectionHash: 'SHA-256 over ordered sourceKey + NUL + contentSha256 + LF records',
      supportTree:
        'all tracked files from each clean detached pin are copied for compiler imported-type and tsconfig resolution; per-repository path/kind/content aggregates and the complete ignored support manifest are reverified before each matrix',
      generatedSupport:
        'committed generated tsconfig artifacts, exact third-party tsconfig packages, and workspace symlinks are copied and rehashed; their generation commands, locks, licenses, and hashes are pinned separately',
    },
    generatedSupport: GENERATED_SUPPORT_SUMMARY,
    summary: scan.summary,
    selections,
    entries: scan.entries,
  };
}

export async function readCorpusManifest(manifestPath) {
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  const repositoryIdentity = ({
    id,
    repository,
    commit,
    licensePath,
    licenseSha256,
    compileAdmissionExclusions,
  }) => ({
    id,
    repository,
    commit,
    licensePath,
    licenseSha256,
    compileAdmissionExclusions,
  });
  if (
    manifest.schema !== 2 ||
    manifest.compiler?.version !== VUE_COMPILER_VERSION ||
    JSON.stringify(manifest.repositories.map(repositoryIdentity)) !==
      JSON.stringify(REPOSITORIES.map(repositoryIdentity))
  ) {
    throw new Error('unsupported or unexpected Vue scale corpus manifest');
  }
  if (JSON.stringify(manifest.generatedSupport) !== JSON.stringify(GENERATED_SUPPORT_SUMMARY)) {
    throw new Error('unexpected generated Vue support summary');
  }
  const sourceKeys = new Set();
  for (const repository of manifest.repositories) {
    if (
      !Number.isSafeInteger(repository.support?.files) ||
      repository.support.files < 1 ||
      !Number.isSafeInteger(repository.support.bytes) ||
      repository.support.bytes < 1 ||
      !Number.isSafeInteger(repository.support.symlinks) ||
      !Number.isSafeInteger(repository.support.gitlinks) ||
      !/^[a-f0-9]{64}$/.test(repository.support.aggregateSha256)
    ) {
      throw new Error(`invalid Vue support summary: ${repository.id}`);
    }
  }
  for (const entry of manifest.entries) {
    if (
      !REPOSITORIES.some(({ id }) => id === entry.repository) ||
      entry.sourceKey !== `${entry.repository}/${entry.path}` ||
      nodePath.posix.isAbsolute(entry.path) ||
      entry.path.split('/').includes('..') ||
      !entry.path.endsWith('.vue') ||
      !Number.isSafeInteger(entry.bytes) ||
      entry.bytes < 1 ||
      !/^[a-f0-9]{64}$/.test(entry.sha256) ||
      !['script-setup', 'ordinary-script', 'template-only'].includes(entry.kind) ||
      sourceKeys.has(entry.sourceKey)
    ) {
      throw new Error(`invalid Vue scale manifest entry: ${JSON.stringify(entry)}`);
    }
    sourceKeys.add(entry.sourceKey);
  }
  const summary = summarizeEntries(manifest.entries);
  assertExpectedCorpus(summary);
  if (JSON.stringify(summary) !== JSON.stringify(manifest.summary)) {
    throw new Error('Vue scale manifest summary is not canonical');
  }
  for (const [count, expectedHash] of Object.entries(FROZEN_SELECTIONS)) {
    const selected = selectManifestEntries(manifest, Number(count));
    const actualSummary = summarizeSelection(selected);
    if (
      actualSummary.selectionSha256 !== expectedHash ||
      JSON.stringify(actualSummary) !== JSON.stringify(manifest.selections[count])
    ) {
      throw new Error(`Vue scale manifest selection mismatch at ${count}`);
    }
  }
  return manifest;
}

export async function prepareCorpus({ sourceRoots, destination, manifestPath, updateManifest }) {
  const scan = await scanSourceCorpora(sourceRoots);
  const generatedSupport = await scanGeneratedSupportTree();
  if (updateManifest) {
    await writeFile(manifestPath, `${JSON.stringify(createCorpusManifest(scan), null, 2)}\n`);
  }
  const manifest = await readCorpusManifest(manifestPath);
  if (JSON.stringify(scan.entries) !== JSON.stringify(manifest.entries)) {
    throw new Error('source checkouts do not match the committed Vue scale corpus manifest');
  }
  if (JSON.stringify(scan.repositories) !== JSON.stringify(manifest.repositories)) {
    throw new Error('source support trees do not match the committed Vue scale manifest');
  }

  const temporaryDestination = `${destination}.tmp-${process.pid}`;
  await rm(temporaryDestination, { recursive: true, force: true });
  try {
    await mkdir(temporaryDestination, { recursive: true });
    for (const repository of REPOSITORIES) {
      for (const entry of scan.supportEntriesByRepository[repository.id]) {
        const sourcePath = nodePath.join(sourceRoots[repository.id], entry.path);
        const destinationPath = nodePath.join(temporaryDestination, repository.id, entry.path);
        await mkdir(nodePath.dirname(destinationPath), { recursive: true });
        if (entry.kind === 'symlink') {
          await symlink(await readlink(sourcePath), destinationPath);
        } else if (entry.kind === 'gitlink') {
          await mkdir(destinationPath, { recursive: true });
        } else {
          await copyFile(sourcePath, destinationPath);
        }
      }
      for (const entry of generatedSupport.entriesByRepository[repository.id]) {
        const destinationPath = nodePath.join(temporaryDestination, repository.id, entry.path);
        await mkdir(nodePath.dirname(destinationPath), { recursive: true });
        if (entry.kind === 'symlink') {
          await symlink(entry.target, destinationPath);
        } else {
          await copyFile(
            nodePath.join(GENERATED_SUPPORT_DIRECTORY, repository.id, entry.path),
            destinationPath,
          );
        }
      }
    }
    await writeFile(
      nodePath.join(temporaryDestination, '.support-manifest.json'),
      `${JSON.stringify(
        {
          schema: 1,
          repositories: Object.fromEntries(
            REPOSITORIES.map(({ id }) => [
              id,
              {
                summary: scan.repositories.find((repository) => repository.id === id).support,
                entries: scan.supportEntriesByRepository[id],
              },
            ]),
          ),
          generated: Object.fromEntries(
            REPOSITORIES.map(({ id }) => [
              id,
              {
                summary: generatedSupport.summaries[id],
                entries: generatedSupport.entriesByRepository[id],
              },
            ]),
          ),
        },
        null,
        2,
      )}\n`,
    );
    await writeFile(
      nodePath.join(temporaryDestination, '.snapshot.json'),
      `${JSON.stringify(
        {
          schema: 1,
          compilerVersion: VUE_COMPILER_VERSION,
          aggregateSha256: manifest.summary.aggregateSha256,
          files: manifest.summary.files,
          generatedSupport: GENERATED_SUPPORT_SUMMARY,
          repositories: Object.fromEntries(
            manifest.repositories.map(({ id, commit, licenseSha256, support }) => [
              id,
              { commit, licenseSha256, support },
            ]),
          ),
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
  const generatedSupport = await scanGeneratedSupportTree();
  const snapshot = JSON.parse(
    await readFile(nodePath.join(corpusDirectory, '.snapshot.json'), 'utf8'),
  );
  const expectedSnapshotRepositories = Object.fromEntries(
    manifest.repositories.map(({ id, commit, licenseSha256, support }) => [
      id,
      { commit, licenseSha256, support },
    ]),
  );
  if (
    snapshot.schema !== 1 ||
    snapshot.compilerVersion !== VUE_COMPILER_VERSION ||
    snapshot.aggregateSha256 !== manifest.summary.aggregateSha256 ||
    snapshot.files !== manifest.summary.files ||
    JSON.stringify(snapshot.generatedSupport) !== JSON.stringify(GENERATED_SUPPORT_SUMMARY) ||
    JSON.stringify(snapshot.repositories) !== JSON.stringify(expectedSnapshotRepositories)
  ) {
    throw new Error('prepared Vue scale corpus snapshot metadata mismatch');
  }
  const supportManifest = JSON.parse(
    await readFile(nodePath.join(corpusDirectory, '.support-manifest.json'), 'utf8'),
  );
  if (supportManifest.schema !== 1) throw new Error('unsupported Vue support manifest');
  for (const repository of REPOSITORIES) {
    const expectedSummary = manifest.repositories.find(({ id }) => id === repository.id).support;
    const preparedSupport = supportManifest.repositories[repository.id];
    if (
      !preparedSupport ||
      JSON.stringify(summarizeSupportEntries(preparedSupport.entries)) !==
        JSON.stringify(expectedSummary) ||
      JSON.stringify(preparedSupport.summary) !== JSON.stringify(expectedSummary)
    ) {
      throw new Error(`prepared Vue support manifest mismatch: ${repository.id}`);
    }
    for (const entry of preparedSupport.entries) {
      const path = nodePath.join(corpusDirectory, repository.id, entry.path);
      const fileStat = await lstat(path);
      const content =
        entry.kind === 'symlink'
          ? Buffer.from(await readlink(path))
          : entry.kind === 'gitlink'
            ? Buffer.from(entry.target)
            : await readFile(path);
      if (
        (entry.kind === 'symlink') !== fileStat.isSymbolicLink() ||
        (entry.kind === 'gitlink') !== fileStat.isDirectory() ||
        content.byteLength !== entry.bytes ||
        sha256(content) !== entry.sha256
      ) {
        throw new Error(`prepared Vue support file mismatch: ${repository.id}/${entry.path}`);
      }
    }
    const preparedGenerated = supportManifest.generated?.[repository.id];
    if (
      !preparedGenerated ||
      JSON.stringify(preparedGenerated.summary) !==
        JSON.stringify(generatedSupport.summaries[repository.id]) ||
      JSON.stringify(preparedGenerated.entries) !==
        JSON.stringify(generatedSupport.entriesByRepository[repository.id])
    ) {
      throw new Error(`prepared generated Vue support manifest mismatch: ${repository.id}`);
    }
    for (const entry of preparedGenerated.entries) {
      const path = nodePath.join(corpusDirectory, repository.id, entry.path);
      const fileStat = await lstat(path);
      const content =
        entry.kind === 'symlink' ? Buffer.from(await readlink(path)) : await readFile(path);
      if (
        (entry.kind === 'symlink') !== fileStat.isSymbolicLink() ||
        content.byteLength !== entry.bytes ||
        sha256(content) !== entry.sha256
      ) {
        throw new Error(
          `prepared generated Vue support file mismatch: ${repository.id}/${entry.path}`,
        );
      }
    }
  }
  for (const entry of manifest.entries) {
    const content = await readFile(nodePath.join(corpusDirectory, entry.sourceKey));
    if (content.byteLength !== entry.bytes || sha256(content) !== entry.sha256) {
      throw new Error(`prepared Vue scale corpus mismatch: ${entry.sourceKey}`);
    }
  }
}

export async function listUnexpectedPreparedFiles(corpusDirectory, manifest) {
  const supportManifest = JSON.parse(
    await readFile(nodePath.join(corpusDirectory, '.support-manifest.json'), 'utf8'),
  );
  const expected = new Set([
    '.snapshot.json',
    '.support-manifest.json',
    ...REPOSITORIES.flatMap(({ id }) =>
      supportManifest.repositories[id].entries.map((entry) => `${id}/${entry.path}`),
    ),
    ...REPOSITORIES.flatMap(({ id }) =>
      supportManifest.generated[id].entries.map((entry) => `${id}/${entry.path}`),
    ),
  ]);
  const actual = [];
  async function walk(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const path = nodePath.join(directory, entry.name);
      if (entry.isDirectory()) await walk(path);
      else actual.push(toPosixPath(nodePath.relative(corpusDirectory, path)));
    }
  }
  await walk(corpusDirectory);
  if (manifest.summary.files !== EXPECTED_CORPUS.files) {
    throw new Error('unexpected Vue manifest while checking prepared files');
  }
  return actual.filter((path) => !expected.has(path)).sort(compareUtf8);
}
