import { readFile } from 'node:fs/promises';
import { captureHarnessSourceManifest, hashBytes } from './harness-provenance.mjs';
import { createCompactAdmissionEvidence, validateAdmissionReport } from './admission-evidence.mjs';
import { assertLocalExecution } from './provenance.mjs';
import { captureVueToolchainProvenance } from './toolchain-provenance.mjs';

assertLocalExecution();
const rawPath = process.argv[2];
const compactPath = process.argv[3];
if (!rawPath || !compactPath) {
  throw new Error('expected <raw-admission.json> <compact-evidence.json>');
}
const rawContent = await readFile(rawPath);
const raw = JSON.parse(rawContent);
const compactContent = await readFile(compactPath);
const compact = JSON.parse(compactContent);
const currentHarness = await captureHarnessSourceManifest();
const vueToolchain = await captureVueToolchainProvenance();
validateAdmissionReport(raw, {
  harnessAggregateSha256: currentHarness.aggregateSha256,
  vueToolchain,
});
const expected = createCompactAdmissionEvidence(rawContent, raw, rawPath, compactPath);
if (JSON.stringify(compact) !== JSON.stringify(expected)) {
  throw new Error('compact Vue admission evidence does not match raw evidence and current sources');
}
console.log(
  JSON.stringify({
    verified: true,
    pointerSha256: hashBytes(compactContent),
    rawSha256: compact.raw.sha256,
    harnessAggregateSha256: currentHarness.aggregateSha256,
  }),
);
