import { realpath } from 'node:fs/promises';
import nodePath from 'node:path';
import {
  listUnexpectedPreparedFiles,
  readCorpusManifest,
  verifyPreparedCorpus,
} from './corpus.mjs';
import { verifyFormalEvidence } from './evidence-verifier.mjs';
import { captureHarnessSourceManifest } from './harness-provenance.mjs';
import { assertLocalExecution } from './provenance.mjs';
import { captureVueToolchainProvenance } from './toolchain-provenance.mjs';

assertLocalExecution();
const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const corpusDirectory = await realpath(nodePath.join(import.meta.dirname, '.corpus'));
const manifest = await readCorpusManifest(
  nodePath.join(import.meta.dirname, 'corpus-manifest.json'),
);
await verifyPreparedCorpus({ corpusDirectory, manifest });
const unexpected = await listUnexpectedPreparedFiles(corpusDirectory, manifest);
if (unexpected.length !== 0) throw new Error(`prepared corpus has unexpected files: ${unexpected}`);
const evidence = await verifyFormalEvidence({
  repositoryRoot,
  harnessSourceManifest: await captureHarnessSourceManifest(),
  vueToolchain: await captureVueToolchainProvenance(),
  manifest,
  corpusDirectory,
});
console.log(JSON.stringify({ verified: true, evidence }));
