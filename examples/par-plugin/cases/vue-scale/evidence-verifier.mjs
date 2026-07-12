import { spawnSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import nodePath from 'node:path';
import { createCompactAdmissionEvidence, validateAdmissionReport } from './admission-evidence.mjs';
import {
  createCompactCorrectnessEvidence,
  PORTABLE_OUTPUT_GOLDEN_FIELDS,
  validateCorrectnessReport,
} from './correctness-evidence.mjs';
import { hashBytes } from './harness-provenance.mjs';

export const CANONICAL_EVIDENCE_PATHS = {
  admission: 'examples/par-plugin/cases/vue-scale/evidence/admission.json',
  correctness: 'examples/par-plugin/cases/vue-scale/evidence/correctness.json',
};
export const CANONICAL_RAW_EVIDENCE_PATHS = {
  admission: 'examples/par-plugin/cases/vue-scale/evidence/raw/admission.json',
  correctness: 'examples/par-plugin/cases/vue-scale/evidence/raw/correctness.json',
};

export async function verifyFormalEvidence({
  repositoryRoot,
  harnessSourceManifest,
  vueToolchain,
  manifest,
  corpusDirectory,
}) {
  const admission = await verifyPointer({
    repositoryRoot,
    relativePath: CANONICAL_EVIDENCE_PATHS.admission,
    expectedRawRelativePath: CANONICAL_RAW_EVIDENCE_PATHS.admission,
    expectedKind: 'vue-scale-admission-evidence-pointer',
  });
  const admissionRaw = JSON.parse(admission.rawContent);
  validateAdmissionReport(admissionRaw, {
    harnessAggregateSha256: harnessSourceManifest.aggregateSha256,
    vueToolchain,
  });
  const expectedAdmission = createCompactAdmissionEvidence(
    admission.rawContent,
    admissionRaw,
    admission.rawPath,
    admission.pointerPath,
  );
  if (JSON.stringify(admission.pointer) !== JSON.stringify(expectedAdmission)) {
    throw new Error('committed Vue admission pointer does not match its raw report');
  }

  const correctness = await verifyPointer({
    repositoryRoot,
    relativePath: CANONICAL_EVIDENCE_PATHS.correctness,
    expectedRawRelativePath: CANONICAL_RAW_EVIDENCE_PATHS.correctness,
    expectedKind: 'vue-scale-correctness-evidence-pointer',
  });
  const correctnessRaw = JSON.parse(correctness.rawContent);
  const goldens = await validateCorrectnessReport(correctnessRaw, {
    harnessSourceManifest,
    vueToolchain,
    manifest,
    corpusDirectory,
  });
  const expectedCorrectness = createCompactCorrectnessEvidence(
    correctness.rawContent,
    correctnessRaw,
    correctness.rawPath,
    correctness.pointerPath,
    goldens,
  );
  if (JSON.stringify(correctness.pointer) !== JSON.stringify(expectedCorrectness)) {
    throw new Error('committed Vue correctness pointer does not match its raw report');
  }
  return {
    admission: evidenceSummary(admission),
    correctness: evidenceSummary(correctness),
    goldens,
  };
}

export function validateOutputAgainstGolden(run, golden) {
  if (!golden?.output) throw new Error(`missing Vue output golden for ${run.componentCount}`);
  for (const field of PORTABLE_OUTPUT_GOLDEN_FIELDS) {
    if (run[field] !== golden.output[field]) {
      throw new Error(
        `formal Vue output differs from correctness golden for ${run.componentCount}/${run.variant}/${field}`,
      );
    }
  }
}

async function verifyPointer({
  repositoryRoot,
  relativePath,
  expectedRawRelativePath,
  expectedKind,
}) {
  const pointerPath = nodePath.join(repositoryRoot, relativePath);
  const pointerContent = await readFile(pointerPath);
  const pointer = JSON.parse(pointerContent);
  if (pointer.schema !== 2 || pointer.kind !== expectedKind || pointer.passed !== true) {
    throw new Error(`invalid committed Vue evidence pointer: ${relativePath}`);
  }
  const trackedContent = gitBuffer(repositoryRoot, ['show', `HEAD:${relativePath}`]);
  if (!trackedContent.equals(pointerContent)) {
    throw new Error(`Vue evidence pointer is not identical to committed HEAD: ${relativePath}`);
  }
  const rawPath = nodePath.resolve(nodePath.dirname(pointerPath), pointer.raw.path);
  const rawRelativePath = nodePath.relative(repositoryRoot, rawPath).split(nodePath.sep).join('/');
  if (rawRelativePath.startsWith('../') || nodePath.posix.isAbsolute(rawRelativePath)) {
    throw new Error(`Vue evidence raw report escapes the fixture repository: ${relativePath}`);
  }
  if (rawRelativePath !== expectedRawRelativePath) {
    throw new Error(`Vue evidence pointer references a non-canonical raw report: ${relativePath}`);
  }
  const rawContent = await readFile(rawPath);
  if (rawContent.byteLength !== pointer.raw.bytes || hashBytes(rawContent) !== pointer.raw.sha256) {
    throw new Error(`Vue evidence raw report differs from pointer: ${relativePath}`);
  }
  const trackedRawContent = gitBuffer(repositoryRoot, ['show', `HEAD:${rawRelativePath}`]);
  if (!trackedRawContent.equals(rawContent)) {
    throw new Error(
      `Vue evidence raw report is not identical to committed HEAD: ${rawRelativePath}`,
    );
  }
  if (!isAncestor(repositoryRoot, pointer.fixtureCommit)) {
    throw new Error(`Vue evidence fixture commit is not an ancestor of HEAD: ${relativePath}`);
  }
  return { pointer, pointerPath, pointerContent, rawPath, rawContent };
}

function evidenceSummary(value) {
  return {
    pointerPath: nodePath.basename(value.pointerPath),
    pointerSha256: hashBytes(value.pointerContent),
    rawPath: value.pointer.raw.path,
    rawBytes: value.rawContent.byteLength,
    rawSha256: hashBytes(value.rawContent),
    fixtureCommit: value.pointer.fixtureCommit,
  };
}

function gitBuffer(root, arguments_) {
  const result = spawnSync('git', ['-C', root, ...arguments_], { encoding: 'buffer' });
  if (result.status !== 0) {
    throw new Error(`git ${arguments_.join(' ')} failed while verifying Vue evidence`);
  }
  return result.stdout;
}

function isAncestor(root, commit) {
  if (!/^[a-f0-9]{40}$/.test(commit ?? '')) return false;
  const result = spawnSync('git', ['-C', root, 'merge-base', '--is-ancestor', commit, 'HEAD']);
  return result.status === 0;
}
