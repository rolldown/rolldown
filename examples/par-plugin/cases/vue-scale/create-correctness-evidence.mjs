import { mkdir, readFile, realpath, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';
import {
  listUnexpectedPreparedFiles,
  readCorpusManifest,
  verifyPreparedCorpus,
} from './corpus.mjs';
import {
  createCompactCorrectnessEvidence,
  validateCorrectnessReport,
} from './correctness-evidence.mjs';
import { captureHarnessSourceManifest } from './harness-provenance.mjs';
import { assertLocalExecution } from './provenance.mjs';
import { captureVueToolchainProvenance } from './toolchain-provenance.mjs';

assertLocalExecution();
const rawPath = process.argv[2];
const outputPath = process.argv[3];
if (!rawPath || !outputPath) {
  throw new Error('expected <raw-correctness-report.json> <compact-evidence.json>');
}
const rawContent = await readFile(rawPath);
const report = JSON.parse(rawContent);
const harnessSourceManifest = await captureHarnessSourceManifest();
const vueToolchain = await captureVueToolchainProvenance();
const corpusDirectory = await realpath(nodePath.join(import.meta.dirname, '.corpus'));
const manifest = await readCorpusManifest(
  nodePath.join(import.meta.dirname, 'corpus-manifest.json'),
);
await verifyPreparedCorpus({ corpusDirectory, manifest });
const unexpected = await listUnexpectedPreparedFiles(corpusDirectory, manifest);
if (unexpected.length !== 0) throw new Error(`prepared corpus has unexpected files: ${unexpected}`);
const goldens = await validateCorrectnessReport(report, {
  harnessSourceManifest,
  vueToolchain,
  manifest,
  corpusDirectory,
});
const compact = createCompactCorrectnessEvidence(rawContent, report, rawPath, outputPath, goldens);
await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(compact, null, 2)}\n`);
console.log(
  JSON.stringify({
    outputPath,
    rawSha256: compact.raw.sha256,
    goldenScales: Object.keys(goldens).map(Number),
    passed: true,
  }),
);
