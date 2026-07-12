import assert from 'node:assert/strict';
import {
  assertExactAdapterProvenance,
  assertFrozenProjectAdapterProvenance,
  captureAdapterBaseProvenance,
  captureProjectAdapterProvenance,
} from './adapter-provenance.mjs';
import { assertLocalNode } from './projects.mjs';
import {
  assertExpectedSubset,
  canonicalEvidenceSha256,
  comparableEvidence,
  createCompactSummary,
  validateMatrix,
  verifyRunOutcome,
} from './verification.mjs';

const definition = {
  projectId: 'fixture',
  ordinaryRepeats: 1,
  workerVariants: ['worker-1'],
  expected: {
    ordinary: { exitCode: 0, executionStatus: 'completed', admissionStatus: 'accepted' },
    worker: { exitCode: 0, executionStatus: 'completed', admissionStatus: 'accepted' },
  },
};
const result = {
  projectId: 'fixture',
  variant: 'ordinary',
  childStatus: 0,
  childSignal: null,
  report: {
    projectId: 'fixture',
    variant: 'ordinary',
    measurementClass: 'correctness-only',
    executionStatus: 'completed',
    admissionStatus: 'accepted',
  },
};

validateMatrix({
  schema: 2,
  measurementClass: 'correctness-only',
  goldenFile: './correctness-goldens.json',
  cases: [definition],
});
verifyRunOutcome(definition, result);
assertExpectedSubset({ transform: { count: 2, hash: 'a' } }, { transform: { hash: 'a' } });

const adapterBase = await captureAdapterBaseProvenance();
const adapter = await captureProjectAdapterProvenance(
  'directus-amendment-candidate',
  '.',
  adapterBase,
);
assert.equal(adapter.compilerSfc.resolutionSource, 'adapter-explicit-option');
assert.equal(adapter.compilerSfc.request, 'vue/compiler-sfc');
assert.equal(adapter.compilerSfc.nodeWrapperPackage.version, '3.5.39');
assert.equal(adapter.compilerSfc.compilerPackage.version, '3.5.39');
assertFrozenProjectAdapterProvenance(adapter, 'directus-amendment-candidate');
const artifactDrift = structuredClone(adapterBase);
artifactDrift.unpluginVue.entrypoint.sha256 = '0'.repeat(64);
assert.throws(
  () => assertExactAdapterProvenance(artifactDrift, adapterBase),
  /adapter provenance drift/,
);

const inheritedNodeOptions = process.env.NODE_OPTIONS;
for (const value of ['', '--no-warnings']) {
  process.env.NODE_OPTIONS = value;
  assert.throws(() => assertLocalNode(), /unset inherited NODE_OPTIONS/);
}
if (inheritedNodeOptions === undefined) delete process.env.NODE_OPTIONS;
else process.env.NODE_OPTIONS = inheritedNodeOptions;

assert.throws(
  () => verifyRunOutcome(definition, { ...result, childStatus: 2 }),
  /exit code 2 != 0/,
);
assert.throws(
  () =>
    verifyRunOutcome(definition, {
      ...result,
      report: { ...result.report, executionStatus: 'failed' },
    }),
  /execution failed != completed/,
);
assert.throws(
  () =>
    verifyRunOutcome(definition, {
      ...result,
      report: { ...result.report, admissionStatus: 'rejected' },
    }),
  /admission rejected != accepted/,
);
assert.throws(
  () => assertExpectedSubset({ transform: { hash: 'drift' } }, { transform: { hash: 'a' } }),
  /evidence\.transform\.hash drift/,
);
assert.throws(
  () =>
    validateMatrix({
      schema: 2,
      measurementClass: 'correctness-only',
      goldenFile: './correctness-goldens.json',
      cases: [{ ...definition, expected: undefined }],
    }),
  /expected\.ordinary is required/,
);

const summary = createCompactSummary(
  {
    node: 'v24.18.0',
    runtime: { profile: { sourceCommit: 'runtime' }, clean: true },
    configuredPools: {},
    matrixSha256: 'matrix',
    goldenSha256: 'golden',
    projectAdmissions: { fixture: 'accepted' },
    results: [
      {
        ...result,
        repeat: 0,
        stderrSha256: 'stderr',
        stdoutSha256: 'stdout',
      },
    ],
  },
  'raw',
  { commit: 'harness', clean: true, sourceManifestSha256: 'source' },
);
assert.equal(summary.rawArtifactSha256, 'raw');
assert.equal(summary.durableEligible, true);
assert.match(summary.canonicalSummarySha256, /^[a-f0-9]{64}$/);

const relocatedReport = (root) => ({
  admissionStatus: 'rejected',
  executionStatus: 'not-run',
  prepared: { root, projectId: 'gitlab' },
  entryProvenance: {
    entries: {
      main: `${root}/app/assets/javascripts/main.js`,
      nested: [`${root}/app/assets/javascripts/nested.js`, { root: `${root}/other.js` }],
    },
    entriesState: {
      watchAutoEntries: [
        `${root}/app/assets/javascripts/pages/projects/index.js`,
        `${root}/app/assets/javascripts/pages/groups/index.js`,
      ],
    },
  },
});
const checkoutA = relocatedReport('/tmp/checkout-a/gitlab');
const checkoutB = relocatedReport('/different/checkout-b/gitlab');
const rawBeforeCanonicalization = JSON.stringify(checkoutA);
assert.equal(canonicalEvidenceSha256(checkoutA), canonicalEvidenceSha256(checkoutB));
assert.equal(JSON.stringify(checkoutA), rawBeforeCanonicalization);
assert.deepEqual(comparableEvidence(checkoutA).entryProvenance.entriesState.watchAutoEntries, [
  '<project-root>/app/assets/javascripts/pages/projects/index.js',
  '<project-root>/app/assets/javascripts/pages/groups/index.js',
]);

console.log('independent Vue verifier negative tests passed');
