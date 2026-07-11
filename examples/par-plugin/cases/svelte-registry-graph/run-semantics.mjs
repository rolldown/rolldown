import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import nodePath from 'node:path';
import { readSourceManifest, verifyGraphCorpus } from './graph-corpus.mjs';
import { hashRolldownDistribution } from './provenance.mjs';

if (process.version !== 'v24.18.0') {
  throw new Error(`registry graph semantics requires Node.js v24.18.0, got ${process.version}`);
}
const outputPath = process.argv[2] ?? nodePath.join(import.meta.dirname, '.results/semantics.json');
const corpusDirectory = nodePath.join(import.meta.dirname, '.graph-corpus');
const manifest = await readSourceManifest(
  nodePath.join(import.meta.dirname, 'source-manifest.json'),
);
await verifyGraphCorpus({ corpusDirectory, manifest });
const entryPaths = manifest.entryPaths.map((path) => nodePath.join(corpusDirectory, path));

const spawnGraph = (variant) => {
  const environment = { ...process.env };
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
  if (variant === 'worker-4') environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS = '4';
  const child = spawnSync(
    process.execPath,
    [
      nodePath.join(import.meta.dirname, 'run-case.mjs'),
      JSON.stringify({
        variant,
        instrumentation: false,
        _corpusDirectory: corpusDirectory,
        _entryPaths: entryPaths,
      }),
    ],
    { encoding: 'utf8', env: environment },
  );
  if (child.status !== 0) throw new Error(`graph warning probe failed:\n${child.stderr}`);
  return JSON.parse(child.stdout);
};
const warningOrdinary = spawnGraph('ordinary');
const warningWorker = spawnGraph('worker-4');
for (const field of ['graphHash', 'outputCodeHash', 'outputMapHash']) {
  if (warningOrdinary[field] !== warningWorker[field]) {
    throw new Error(`warning graph ${field} differs`);
  }
}

const errorFixture = await mkdtemp(nodePath.join(tmpdir(), 'rolldown-svelte-graph-error-'));
let errorOrdinary;
let errorWorker;
try {
  const entryDirectory = nodePath.join(errorFixture, 'docs/src/lib/registry/ui/broken');
  await mkdir(entryDirectory, { recursive: true });
  const entryPath = nodePath.join(entryDirectory, 'index.ts');
  await writeFile(entryPath, "export { default as Broken } from './broken.svelte';\n");
  await writeFile(nodePath.join(entryDirectory, 'broken.svelte'), '{#if true}<p>broken{/each}\n');
  const spawnError = (variant) => {
    const environment = { ...process.env };
    delete environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
    if (variant === 'worker-2') environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS = '2';
    const child = spawnSync(
      process.execPath,
      [
        nodePath.join(import.meta.dirname, 'run-error-case.mjs'),
        JSON.stringify({ variant, fixtureDirectory: errorFixture, entryPath }),
      ],
      { encoding: 'utf8', env: environment },
    );
    if (child.status !== 0) throw new Error(`error probe process failed:\n${child.stderr}`);
    return JSON.parse(child.stdout);
  };
  errorOrdinary = spawnError('ordinary');
  errorWorker = spawnError('worker-2');
} finally {
  await rm(errorFixture, { recursive: true, force: true });
}
if (errorOrdinary.success || errorWorker.success) {
  throw new Error('invalid reached Svelte module unexpectedly built');
}

const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const nativeBindingSourceCommit = '54fd0e24112505443044a4bba5c41d1f4d9ba2aa';
const git = (arguments_) => {
  const result = spawnSync('git', ['-C', repositoryRoot, ...arguments_], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ${arguments_.join(' ')} failed`);
  return result.stdout.trim();
};
const worktreeStatus = git(['status', '--short']);
if (worktreeStatus) throw new Error('final graph semantics requires a clean worktree');
const bindingDirectory = nodePath.join(repositoryRoot, 'packages/rolldown/src');
const bindingNames = (await readdir(bindingDirectory)).filter((name) =>
  /^rolldown-binding\..+\.node$/.test(name),
);
if (bindingNames.length !== 1) throw new Error('expected one native binding');
const bindingPath = nodePath.join(bindingDirectory, bindingNames[0]);
const bindingContent = await readFile(bindingPath);
const bindingStat = await stat(bindingPath);
const nodeContent = await readFile(process.execPath);
const nodeStat = await stat(process.execPath);
const report = {
  schema: 1,
  timestamp: new Date().toISOString(),
  node: process.version,
  nodeBinary: process.execPath,
  nodeArtifact: {
    bytes: nodeStat.size,
    sha256: createHash('sha256').update(nodeContent).digest('hex'),
  },
  rolldownCommit: git(['rev-parse', 'HEAD']),
  rolldownWorktreeStatus: worktreeStatus,
  nativeBinding: {
    path: nodePath.relative(repositoryRoot, bindingPath),
    bytes: bindingStat.size,
    sha256: createHash('sha256').update(bindingContent).digest('hex'),
    sourceCommit: nativeBindingSourceCommit,
  },
  rolldownDistribution: await hashRolldownDistribution(repositoryRoot),
  graphWarnings: {
    ordinary: warningOrdinary.logs,
    worker: warningWorker.logs,
    sameLogs: JSON.stringify(warningOrdinary.logs) === JSON.stringify(warningWorker.logs),
    outputCodeHash: warningOrdinary.outputCodeHash,
    outputMapHash: warningOrdinary.outputMapHash,
  },
  invalidReachedSvelte: {
    ordinary: errorOrdinary,
    worker: errorWorker,
    sameError: JSON.stringify(errorOrdinary.error) === JSON.stringify(errorWorker.error),
  },
};
await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(
  JSON.stringify({
    outputPath,
    graphWarnings: warningOrdinary.logs.length,
    sameLogs: report.graphWarnings.sameLogs,
    sameError: report.invalidReachedSvelte.sameError,
  }),
);
