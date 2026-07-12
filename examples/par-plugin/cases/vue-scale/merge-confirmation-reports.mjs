import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';
import { assertLocalExecution } from './provenance.mjs';

assertLocalExecution();
const priorPath = process.argv[2];
const additionalPath = process.argv[3];
const outputPath = process.argv[4];
if (!priorPath || !additionalPath || !outputPath) {
  throw new Error(
    'expected <prior-confirmation.json> <additional-confirmation.json> <merged.json>',
  );
}
const priorContent = await readFile(priorPath);
const additionalContent = await readFile(additionalPath);
const prior = JSON.parse(priorContent);
const additional = JSON.parse(additionalContent);
for (const report of [prior, additional]) {
  if (
    report.schema !== 1 ||
    report.matrix?.lane !== 'wall-confirm' ||
    report.measurementClass !== 'formal local wall evidence subject to host gates' ||
    report.admitted !== true ||
    report.admissionFailures?.length !== 0
  ) {
    throw new Error('confirmation merge requires two admitted wall-confirm reports');
  }
}
for (const field of ['runtime', 'harnessSourceManifest', 'corpus']) {
  if (JSON.stringify(prior[field]) !== JSON.stringify(additional[field])) {
    throw new Error(`confirmation reports disagree on ${field}`);
  }
}
const expectedPriorHash = additional.matrix.generatedFrom?.priorConfirmationSha256;
if (expectedPriorHash !== createHash('sha256').update(priorContent).digest('hex')) {
  throw new Error('additional confirmation is not pinned to the supplied prior report');
}
const maxIndexByScale = new Map();
for (const run of prior.runs) {
  maxIndexByScale.set(
    run.componentCount,
    Math.max(maxIndexByScale.get(run.componentCount) ?? -1, run.index),
  );
}
const reindexedAdditionalRuns = additional.runs.map((run) => ({
  ...run,
  index: run.index + (maxIndexByScale.get(run.componentCount) ?? -1) + 1,
}));
const runs = [...prior.runs, ...reindexedAdditionalRuns].map((run, sequence) => ({
  ...run,
  sequence,
}));
const caseSelections = [
  ...new Map(
    [...prior.caseSelections, ...additional.caseSelections].map((selection) => [
      selection.componentCount,
      selection,
    ]),
  ).values(),
].sort((left, right) => left.componentCount - right.componentCount);
const merged = {
  ...prior,
  startedAt: prior.startedAt,
  finishedAt: additional.finishedAt,
  hostAdmissions: [...prior.hostAdmissions, ...additional.hostAdmissions],
  matrix: {
    ...prior.matrix,
    description:
      'Merged iterative wall confirmation; every source report and reindexing step is pinned below.',
    cases: [...prior.matrix.cases, ...additional.matrix.cases],
    mergedIterations: [
      ...(prior.matrix.mergedIterations ?? [
        { path: priorPath, sha256: createHash('sha256').update(priorContent).digest('hex') },
      ]),
      {
        path: additionalPath,
        sha256: createHash('sha256').update(additionalContent).digest('hex'),
      },
    ],
  },
  caseSelections,
  runs,
};
await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(merged, null, 2)}\n`);
console.log(JSON.stringify({ outputPath, scales: caseSelections.length, runs: runs.length }));
