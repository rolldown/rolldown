import { spawnSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';
import {
  QUASAR_PRE_EXCLUSION,
  createCompactAdmissionEvidence,
  validateAdmissionReport,
} from './admission-evidence.mjs';
import { listQuasarPreExclusionEntries, summarizeAdmissionEntries } from './admission-corpus.mjs';
import {
  listUnexpectedPreparedFiles,
  readCorpusManifest,
  verifyPreparedCorpus,
} from './corpus.mjs';
import { captureHarnessSourceManifest } from './harness-provenance.mjs';
import { captureVueToolchainProvenance } from './toolchain-provenance.mjs';
import {
  BASELINE_POOL_ENVIRONMENT,
  LIFECYCLE_BASELINE_DISTRIBUTION_SHA256,
  LIFECYCLE_BASELINE_NATIVE_BINDING_SHA256,
  LIFECYCLE_BASELINE_SOURCE_COMMIT,
  assertLocalExecution,
  assertRuntimeStable,
  inspectRuntimeProvenance,
} from './provenance.mjs';

assertLocalExecution();
const rawPath = process.argv[2];
const compactPath = process.argv[3];
const validateOnly = process.argv.includes('--validate-only');
if (!rawPath || !compactPath) {
  throw new Error('expected <raw-admission.json> <compact-evidence.json> [rolldown-package-root]');
}
const fixtureRepositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const runtimePackageRoot = nodePath.resolve(
  process.argv.slice(4).find((argument) => !argument.startsWith('--')) ??
    nodePath.join(fixtureRepositoryRoot, 'packages/rolldown'),
);
const runtimeRepositoryRoot = nodePath.resolve(runtimePackageRoot, '../..');
const runtimePin = {
  kind: 'lifecycle-corrected-baseline',
  sourceCommit: LIFECYCLE_BASELINE_SOURCE_COMMIT,
  nativeBindingSha256: LIFECYCLE_BASELINE_NATIVE_BINDING_SHA256,
  distributionSha256: LIFECYCLE_BASELINE_DISTRIBUTION_SHA256,
};
const runtime = await inspectRuntimeProvenance(runtimeRepositoryRoot, runtimePackageRoot, {
  requireClean: !validateOnly,
  expectedPin: runtimePin,
});
const fixtureStatusResult = spawnSync('git', ['-C', fixtureRepositoryRoot, 'status', '--short'], {
  encoding: 'utf8',
});
if (fixtureStatusResult.status !== 0) throw new Error('failed to inspect fixture worktree status');
const fixtureStatus = fixtureStatusResult.stdout.trim();
if (!validateOnly && fixtureStatus) {
  throw new Error('Vue admission audit requires a clean fixture worktree');
}
const corpusDirectory = nodePath.join(import.meta.dirname, '.corpus');
const manifest = await readCorpusManifest(
  nodePath.join(import.meta.dirname, 'corpus-manifest.json'),
);
await verifyPreparedCorpus({ corpusDirectory, manifest });
const unexpected = await listUnexpectedPreparedFiles(corpusDirectory, manifest);
if (unexpected.length !== 0) throw new Error(`prepared corpus has unexpected files: ${unexpected}`);
const harnessSourceManifest = await captureHarnessSourceManifest();
const vueToolchain = await captureVueToolchainProvenance();
const quasarSelection = summarizeAdmissionEntries(
  await listQuasarPreExclusionEntries(corpusDirectory),
);
for (const [field, expected] of Object.entries(QUASAR_PRE_EXCLUSION)) {
  if (quasarSelection[field] !== expected) {
    throw new Error(`Quasar pre-exclusion source scan ${field} mismatch`);
  }
}
if (validateOnly) {
  console.log(
    JSON.stringify({
      validatedOnly: true,
      runtimePin,
      harnessAggregateSha256: harnessSourceManifest.aggregateSha256,
      vueToolchain,
      quasarPreExclusion: quasarSelection,
      finalPool: manifest.summary,
    }),
  );
  process.exit(0);
}
const audits = ['quasar-pre-exclusion', 'final-pool'].map(runPhase);
await assertRuntimeStable(runtimeRepositoryRoot, runtimePackageRoot, runtime);
if (
  JSON.stringify(await captureHarnessSourceManifest()) !== JSON.stringify(harnessSourceManifest)
) {
  throw new Error('Vue harness changed during admission audit');
}
const report = {
  schema: 1,
  kind: 'vue-scale-admission-audit',
  measurementClass: 'untimed compile admission; not performance evidence',
  runtime,
  harnessSourceManifest,
  vueToolchain,
  fixture: {
    commit: git(['rev-parse', 'HEAD']),
    worktreeStatus: fixtureStatus,
  },
  executionEnvironment: {
    inheritedNodeOptions: null,
    childNodeOptions: `--import=${nodePath.join(import.meta.dirname, 'register-loader.mjs')}`,
    configuredPools: BASELINE_POOL_ENVIRONMENT,
  },
  corpus: {
    compiler: manifest.compiler,
    repositories: manifest.repositories,
    summary: manifest.summary,
  },
  audits,
};
validateAdmissionReport(report, {
  harnessAggregateSha256: harnessSourceManifest.aggregateSha256,
  vueToolchain,
});
const rawContent = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
const compact = createCompactAdmissionEvidence(rawContent, report, rawPath, compactPath);
await mkdir(nodePath.dirname(nodePath.resolve(rawPath)), { recursive: true });
await mkdir(nodePath.dirname(nodePath.resolve(compactPath)), { recursive: true });
await writeFile(rawPath, rawContent);
await writeFile(compactPath, `${JSON.stringify(compact, null, 2)}\n`);
console.log(
  JSON.stringify({
    rawPath,
    compactPath,
    rawSha256: compact.raw.sha256,
    quasarErrors: audits[0].errorCount,
    finalPoolFiles: audits[1].selection.files,
    passed: true,
  }),
);

function runPhase(phase) {
  const environment = { ...process.env, ...BASELINE_POOL_ENVIRONMENT };
  environment.ROLLDOWN_RESEARCH_PACKAGE_ROOT = runtimePackageRoot;
  environment.NODE_OPTIONS = `--import=${nodePath.join(import.meta.dirname, 'register-loader.mjs')}`;
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS;
  const result = spawnSync(
    process.execPath,
    [
      nodePath.join(import.meta.dirname, 'run-admission-case.mjs'),
      JSON.stringify({ phase, corpusDirectory }),
    ],
    { encoding: 'utf8', env: environment, maxBuffer: 4 * 1024 * 1024 },
  );
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `Vue admission phase ${phase} failed with status ${result.status}: ${result.stderr}`,
    );
  }
  if (result.stderr.trim() !== '') {
    throw new Error(`Vue admission phase ${phase} emitted unexpected stderr: ${result.stderr}`);
  }
  return JSON.parse(result.stdout);
}

function git(arguments_) {
  const result = spawnSync('git', ['-C', fixtureRepositoryRoot, ...arguments_], {
    encoding: 'utf8',
  });
  if (result.status !== 0) throw new Error(`git ${arguments_.join(' ')} failed`);
  return result.stdout.trim();
}
