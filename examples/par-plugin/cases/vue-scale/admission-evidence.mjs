import nodePath from 'node:path';
import { EXPECTED_CORPUS } from './corpus.mjs';
import { hashBytes } from './harness-provenance.mjs';

export const QUASAR_PRE_EXCLUSION = {
  files: 1112,
  bytes: 2245255,
  uniqueContents: 1112,
  aggregateSha256: '49f5089abac134b76c7e9ee6e21db1c073ebcdf85e1cd46d65c3ef82fe36945d',
};
export const EXPECTED_QUASAR_FAILURES = [
  'quasar/app-vite/playground-ts/src/components/EssentialLink.vue',
  'quasar/app-vite/playground-ts/src/pages/index/(index).vue',
  'quasar/app-vite/playground-ts/src/pages/index/second.vue',
];

export function validateAdmissionReport(report, { harnessAggregateSha256, vueToolchain }) {
  if (
    report?.schema !== 1 ||
    report.kind !== 'vue-scale-admission-audit' ||
    report.measurementClass !== 'untimed compile admission; not performance evidence' ||
    report.harnessSourceManifest?.aggregateSha256 !== harnessAggregateSha256 ||
    JSON.stringify(report.vueToolchain) !== JSON.stringify(vueToolchain) ||
    report.runtime?.worktreeStatus !== '' ||
    report.fixture?.worktreeStatus !== '' ||
    report.executionEnvironment?.inheritedNodeOptions !== null
  ) {
    throw new Error('invalid Vue admission evidence header');
  }
  const quasar = report.audits?.find(({ phase }) => phase === 'quasar-pre-exclusion');
  const full = report.audits?.find(({ phase }) => phase === 'final-pool');
  if (!quasar || !full || report.audits.length !== 2) {
    throw new Error('Vue admission evidence must contain both frozen audits');
  }
  for (const [field, expected] of Object.entries(QUASAR_PRE_EXCLUSION)) {
    const actualField = field === 'aggregateSha256' ? 'aggregateSha256' : field;
    if (quasar.selection[actualField] !== expected) {
      throw new Error(`Quasar pre-exclusion ${field} mismatch`);
    }
  }
  const failureIds = quasar.failures.map(({ sourceKey }) => sourceKey).sort();
  if (
    quasar.admitted !== false ||
    quasar.errorCount !== EXPECTED_QUASAR_FAILURES.length ||
    JSON.stringify(failureIds) !== JSON.stringify(EXPECTED_QUASAR_FAILURES) ||
    quasar.failures.some(
      ({ code, signature }) =>
        code !== 'TSCONFIG_ERROR' ||
        signature !== 'TSCONFIG_ERROR: missing dependency of nearest tsconfig',
    )
  ) {
    throw new Error('Quasar pre-exclusion failure set is not the frozen three-path set');
  }
  for (const [field, expected] of Object.entries(EXPECTED_CORPUS)) {
    if (full.selection[field] !== expected) {
      throw new Error(`full Vue admission ${field} mismatch`);
    }
  }
  if (
    full.admitted !== true ||
    full.errorCount !== 0 ||
    full.failures.length !== 0 ||
    full.output?.exports !== EXPECTED_CORPUS.files ||
    typeof full.output?.normalizedCodeSha256 !== 'string' ||
    typeof full.output?.normalizedMapSha256 !== 'string'
  ) {
    throw new Error('full 5,650-source Vue pool did not pass ordinary compile and generate');
  }
}

export function createCompactAdmissionEvidence(rawContent, rawReport, rawPath, compactPath) {
  return {
    schema: 2,
    kind: 'vue-scale-admission-evidence-pointer',
    passed: true,
    measurementClass: rawReport.measurementClass,
    raw: {
      path: nodePath
        .relative(nodePath.dirname(nodePath.resolve(compactPath)), nodePath.resolve(rawPath))
        .split(nodePath.sep)
        .join('/'),
      bytes: rawContent.byteLength,
      sha256: hashBytes(rawContent),
    },
    harnessSourceManifest: {
      files: rawReport.harnessSourceManifest.files,
      bytes: rawReport.harnessSourceManifest.bytes,
      aggregateSha256: rawReport.harnessSourceManifest.aggregateSha256,
    },
    runtimePin: rawReport.runtime.runtimePin,
    vueToolchain: rawReport.vueToolchain,
    fixtureCommit: rawReport.fixture.commit,
    corpusAggregateSha256: rawReport.corpus.summary.aggregateSha256,
    quasarPreExclusion: {
      files: rawReport.audits[0].selection.files,
      errors: rawReport.audits[0].errorCount,
      failureIds: rawReport.audits[0].failures.map(({ sourceKey }) => sourceKey),
    },
    finalPool: {
      files: rawReport.audits[1].selection.files,
      admitted: rawReport.audits[1].admitted,
      normalizedCodeSha256: rawReport.audits[1].output.normalizedCodeSha256,
      normalizedMapSha256: rawReport.audits[1].output.normalizedMapSha256,
    },
  };
}
