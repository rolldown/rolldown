import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdir, readFile, readdir, stat, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';
import { readSourceManifest, verifyGraphCorpus } from './graph-corpus.mjs';
import { hashRolldownDistribution } from './provenance.mjs';

if (process.version !== 'v24.18.0') {
  throw new Error(`registry graph proof requires Node.js v24.18.0, got ${process.version}`);
}
const updateExpected = process.argv.includes('--update-expected');
const outputArgument = process.argv.slice(2).find((argument) => !argument.startsWith('--'));
const outputPath =
  outputArgument ?? nodePath.join(import.meta.dirname, '.results/ordinary-proof.json');
const manifestPath = nodePath.join(import.meta.dirname, 'source-manifest.json');
const expectedPath = nodePath.join(import.meta.dirname, 'expected-graph.json');
const corpusDirectory = nodePath.join(import.meta.dirname, '.graph-corpus');
const manifest = await readSourceManifest(manifestPath);
const snapshot = await verifyGraphCorpus({ corpusDirectory, manifest });
const entryPaths = manifest.entryPaths.map((path) => nodePath.join(corpusDirectory, path));
const child = spawnSync(
  process.execPath,
  [
    '--expose-gc',
    nodePath.join(import.meta.dirname, 'run-case.mjs'),
    JSON.stringify({
      variant: 'ordinary',
      instrumentation: true,
      _corpusDirectory: corpusDirectory,
      _entryPaths: entryPaths,
    }),
  ],
  { encoding: 'utf8', env: { ...process.env } },
);
if (child.status !== 0) {
  throw new Error(
    `ordinary registry graph proof failed with status ${child.status}:\n${child.stderr}`,
  );
}
const run = JSON.parse(child.stdout);
if (run.projectLocalExternalCount !== 0) throw new Error('ordinary proof externalized local code');
if (run.componentModuleCount !== run.jsMetrics.componentCalls) {
  throw new Error('component graph count and transform calls differ');
}
if (run.svelteModuleCount !== run.jsMetrics.moduleCalls) {
  throw new Error('Svelte module graph count and compileModule calls differ');
}
for (const entryPath of manifest.entryPaths) {
  if (!run.localModulePaths.includes(entryPath))
    throw new Error(`entry was not reached: ${entryPath}`);
}
const sourceByPath = new Map(snapshot.entries.map((entry) => [entry.path, entry]));
const expectedTransformInputBytes = run.localModulePaths
  .filter((path) => path.endsWith('.svelte') || /\.svelte\.(?:js|ts)$/.test(path))
  .reduce((total, path) => total + sourceByPath.get(path).bytes, 0);
if (run.jsMetrics.handlerInputCodeBytes !== expectedTransformInputBytes) {
  throw new Error('ordinary proof transform input bytes differ from reached source files');
}
const expected = {
  schema: 1,
  sourceAggregateSha256: manifest.summary.aggregateSha256,
  entryPaths: manifest.entryPaths,
  localModulePaths: run.localModulePaths,
  localModuleCount: run.localModuleCount,
  componentModuleCount: run.componentModuleCount,
  svelteModuleCount: run.svelteModuleCount,
  typeScriptModuleCount: run.typeScriptModuleCount,
  graphSourceBytes: run.graphSourceBytes,
  graphHash: run.graphHash,
  expectedTransformInputBytes,
  componentCalls: run.jsMetrics.componentCalls,
  moduleCalls: run.jsMetrics.moduleCalls,
  outputChunkCount: run.outputChunkCount,
  outputAssetCount: run.outputAssetCount,
  outputCodeBytes: run.outputCodeBytes,
  outputMapBytes: run.outputMapBytes,
  nullMapChunkCount: run.nullMapChunkCount,
  totalExports: run.totalExports,
  outputCodeHash: run.outputCodeHash,
  outputMapHash: run.outputMapHash,
  resolverTelemetry: run.resolverTelemetry,
  bareExternalIds: run.bareExternalIds,
  appVirtualExternals: run.appVirtualExternals,
  workspacePackageExternals: run.workspacePackageExternals,
  svelteRuntimeExternals: run.svelteRuntimeExternals,
  thirdPartyBareExternals: run.thirdPartyBareExternals,
  bareExternalPackages: run.bareExternalPackages,
  externalizedImportCount: run.externalizedImports.length,
  logs: run.logs,
};
if (updateExpected) {
  await writeFile(expectedPath, `${JSON.stringify(expected, null, 2)}\n`);
} else {
  const committedExpected = JSON.parse(await readFile(expectedPath, 'utf8'));
  if (JSON.stringify(expected) !== JSON.stringify(committedExpected)) {
    throw new Error('ordinary proof differs from expected-graph.json');
  }
}

const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const nativeBindingSourceCommit = '54fd0e24112505443044a4bba5c41d1f4d9ba2aa';
const bindingDirectory = nodePath.join(repositoryRoot, 'packages/rolldown/src');
const bindingNames = (await readdir(bindingDirectory)).filter((name) =>
  /^rolldown-binding\..+\.node$/.test(name),
);
if (bindingNames.length !== 1) throw new Error('expected one local native binding');
const bindingPath = nodePath.join(bindingDirectory, bindingNames[0]);
const bindingContent = await readFile(bindingPath);
const bindingStat = await stat(bindingPath);
const nodeBinaryContent = await readFile(process.execPath);
const nodeBinaryStat = await stat(process.execPath);
const git = (arguments_) => {
  const result = spawnSync('git', ['-C', repositoryRoot, ...arguments_], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ${arguments_.join(' ')} failed`);
  return result.stdout.trim();
};
const report = {
  schema: 1,
  timestamp: new Date().toISOString(),
  node: process.version,
  nodeBinary: process.execPath,
  nodeArtifact: {
    bytes: nodeBinaryStat.size,
    sha256: createHash('sha256').update(nodeBinaryContent).digest('hex'),
  },
  svelteVersion: '5.56.4',
  rolldownCommit: git(['rev-parse', 'HEAD']),
  rolldownWorktreeStatus: git(['status', '--short']),
  nativeBinding: {
    path: nodePath.relative(repositoryRoot, bindingPath),
    bytes: bindingStat.size,
    sha256: createHash('sha256').update(bindingContent).digest('hex'),
    sourceCommit: nativeBindingSourceCommit,
  },
  rolldownDistribution: await hashRolldownDistribution(repositoryRoot),
  manifest,
  expected,
  run,
};
if (!updateExpected && report.rolldownWorktreeStatus) {
  throw new Error('final ordinary graph proof requires a clean worktree');
}
await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(
  JSON.stringify({
    outputPath,
    updatedExpected: updateExpected,
    localModules: run.localModuleCount,
    components: run.componentModuleCount,
    svelteModules: run.svelteModuleCount,
    outputCodeHash: run.outputCodeHash,
    outputMapHash: run.outputMapHash,
  }),
);
