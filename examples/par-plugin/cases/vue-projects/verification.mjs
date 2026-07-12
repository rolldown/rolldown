import { createHash } from 'node:crypto';
import { assertFrozenProjectAdapterProvenance } from './adapter-provenance.mjs';

const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const OUTCOMES = new Set(['accepted', 'rejected']);
const EXECUTIONS = new Set(['completed', 'failed', 'not-run']);

export function assertExpectedSubset(actual, expected, path = 'evidence') {
  if (Array.isArray(expected)) {
    if (!Array.isArray(actual) || actual.length !== expected.length) {
      throw new Error(`${path} array drift`);
    }
    for (let index = 0; index < expected.length; index++) {
      assertExpectedSubset(actual[index], expected[index], `${path}[${index}]`);
    }
    return;
  }
  if (expected && typeof expected === 'object') {
    if (!actual || typeof actual !== 'object') throw new Error(`${path} object missing`);
    for (const [key, value] of Object.entries(expected)) {
      assertExpectedSubset(actual[key], value, `${path}.${key}`);
    }
    return;
  }
  if (!Object.is(actual, expected)) {
    throw new Error(`${path} drift: ${JSON.stringify(actual)} != ${JSON.stringify(expected)}`);
  }
}

function validateOutcome(value, path) {
  if (!value || typeof value !== 'object') throw new Error(`${path} is required`);
  if (!Number.isInteger(value.exitCode)) throw new Error(`${path}.exitCode must be an integer`);
  if (!EXECUTIONS.has(value.executionStatus)) {
    throw new Error(`${path}.executionStatus is invalid`);
  }
  if (!OUTCOMES.has(value.admissionStatus)) {
    throw new Error(`${path}.admissionStatus is invalid`);
  }
  if (value.executionStatus !== 'completed' && value.admissionStatus !== 'rejected') {
    throw new Error(`${path} cannot accept an execution that did not complete`);
  }
  if ((value.executionStatus === 'failed') !== (value.exitCode !== 0)) {
    throw new Error(`${path} exitCode does not match failed execution status`);
  }
}

export function validateMatrix(matrix) {
  if (matrix.schema !== 2) throw new Error('matrix schema must be 2');
  if (matrix.measurementClass !== 'correctness-only') {
    throw new Error('matrix measurementClass must be correctness-only');
  }
  if (typeof matrix.goldenFile !== 'string' || matrix.goldenFile.length === 0) {
    throw new Error('matrix.goldenFile is required');
  }
  if (!Array.isArray(matrix.cases)) throw new Error('matrix.cases must be an array');
  const ids = new Set();
  for (const [index, definition] of matrix.cases.entries()) {
    const path = `matrix.cases[${index}]`;
    if (typeof definition.projectId !== 'string') throw new Error(`${path}.projectId is required`);
    if (ids.has(definition.projectId)) throw new Error(`${path}.projectId is duplicated`);
    ids.add(definition.projectId);
    if (!Number.isInteger(definition.ordinaryRepeats) || definition.ordinaryRepeats < 1) {
      throw new Error(`${path}.ordinaryRepeats must be a positive integer`);
    }
    if (!Array.isArray(definition.workerVariants)) {
      throw new Error(`${path}.workerVariants must be an array`);
    }
    if (new Set(definition.workerVariants).size !== definition.workerVariants.length) {
      throw new Error(`${path}.workerVariants contains duplicates`);
    }
    for (const variant of definition.workerVariants) {
      if (!/^worker-[1-8]$/.test(variant)) throw new Error(`${path} has invalid ${variant}`);
    }
    validateOutcome(definition.expected?.ordinary, `${path}.expected.ordinary`);
    if (definition.workerVariants.length !== 0) {
      validateOutcome(definition.expected?.worker, `${path}.expected.worker`);
    }
  }
}

export function expectedOutcome(definition, variant) {
  return variant === 'ordinary' ? definition.expected.ordinary : definition.expected.worker;
}

export function verifyRunOutcome(definition, result) {
  const expected = expectedOutcome(definition, result.variant);
  if (result.childSignal !== null) {
    throw new Error(`${result.projectId}/${result.variant} exited by signal ${result.childSignal}`);
  }
  if (result.childStatus !== expected.exitCode) {
    throw new Error(
      `${result.projectId}/${result.variant} exit code ${result.childStatus} != ${expected.exitCode}`,
    );
  }
  if (!result.report) throw new Error(`${result.projectId}/${result.variant} emitted no report`);
  if (result.report.projectId !== result.projectId || result.report.variant !== result.variant) {
    throw new Error(`${result.projectId}/${result.variant} report identity drift`);
  }
  if (result.report.measurementClass !== 'correctness-only') {
    throw new Error(`${result.projectId}/${result.variant} emitted timing evidence`);
  }
  if (result.report.executionStatus !== expected.executionStatus) {
    throw new Error(
      `${result.projectId}/${result.variant} execution ${result.report.executionStatus} != ${expected.executionStatus}`,
    );
  }
  if (result.report.admissionStatus !== expected.admissionStatus) {
    throw new Error(
      `${result.projectId}/${result.variant} admission ${result.report.admissionStatus} != ${expected.admissionStatus}`,
    );
  }
}

export function stablePrepared(prepared) {
  if (!prepared) return prepared;
  const dependencyPreparation = prepared.dependencyPreparation
    ? {
        ...prepared.dependencyPreparation,
        invokedThrough: '<node-bin>/corepack',
        installPerformed: undefined,
      }
    : undefined;
  return { ...prepared, root: '<project-root>', dependencyPreparation };
}

function normalizeProjectRootPaths(value, projectRoot) {
  if (!projectRoot) return value;
  if (typeof value === 'string') return value.replaceAll(projectRoot, '<project-root>');
  if (Array.isArray(value)) {
    return value.map((entry) => normalizeProjectRootPaths(entry, projectRoot));
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [
        key,
        normalizeProjectRootPaths(entry, projectRoot),
      ]),
    );
  }
  return value;
}

export function comparableEvidence(report) {
  return {
    admissionStatus: report.admissionStatus,
    executionStatus: report.executionStatus,
    prepared: stablePrepared(report.prepared),
    entryProvenance: normalizeProjectRootPaths(report.entryProvenance, report.prepared?.root),
    adapterProvenance: report.adapterProvenance,
    compilerContract: report.compilerContract,
    transform: report.transform,
    graph: report.graph,
    warnings: report.warnings,
    output: report.output,
    rejection: report.rejection,
    admissionFailures: report.admissionFailures,
    capabilityBoundary: report.capabilityBoundary,
  };
}

export function verifyGolden(projectId, report, goldens) {
  if (goldens.schema !== 1) throw new Error('golden schema must be 1');
  const projectGolden = goldens.projects?.[projectId];
  if (!projectGolden) throw new Error(`missing correctness golden for ${projectId}`);
  assertFrozenProjectAdapterProvenance(report.adapterProvenance, projectId);
  assertExpectedSubset(report, goldens.shared, `${projectId}.shared`);
  assertExpectedSubset(report, projectGolden, `${projectId}.golden`);
}

export function canonicalEvidenceSha256(report) {
  return sha256(JSON.stringify(comparableEvidence(report)));
}

export function createCompactSummary(report, rawArtifactSha256, harness) {
  const artifactRunHashes = [];
  const runs = report.results.map((result) => {
    if (result.skipped) return result;
    artifactRunHashes.push({
      projectId: result.projectId,
      variant: result.variant,
      repeat: result.repeat,
      stdoutSha256: result.stdoutSha256,
      stderrSha256: result.stderrSha256,
    });
    return {
      projectId: result.projectId,
      variant: result.variant,
      repeat: result.repeat,
      exitCode: result.childStatus,
      executionStatus: result.report.executionStatus,
      admissionStatus: result.report.admissionStatus,
      canonicalEvidenceSha256: canonicalEvidenceSha256(result.report),
    };
  });
  const canonical = {
    schema: 1,
    measurementClass: 'correctness-only',
    timingEligible: false,
    node: report.node,
    runtime: report.runtime.profile,
    configuredPools: report.configuredPools,
    matrixSha256: report.matrixSha256,
    goldenSha256: report.goldenSha256,
    projectAdmissions: report.projectAdmissions,
    harness,
    adapterToolchain: report.adapterToolchain,
    projectAdapterProvenance: report.projectAdapterProvenance,
    executionEnvironment: report.executionEnvironment,
    runs,
  };
  return {
    ...canonical,
    durableEligible: harness.clean && report.runtime?.clean === true,
    canonicalSummarySha256: sha256(JSON.stringify(canonical)),
    artifactRunHashes,
    rawArtifactSha256,
    rawHashScope:
      'SHA-256 of the exact uncommitted raw JSON artifact bytes; timestamps and host fields mean a later run has a different raw hash.',
    canonicalHashScope:
      'SHA-256 of outcome and normalized correctness evidence. Regenerate only from a clean committed harness and retain the raw artifact beside this summary.',
  };
}
