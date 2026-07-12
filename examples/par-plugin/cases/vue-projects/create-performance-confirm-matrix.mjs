import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';
import {
  createConfirmationMatrixFromScreen,
  validatePerformanceMatrix,
} from './performance-policy.mjs';
import { assertLocalNode } from './projects.mjs';

assertLocalNode();
const screenPath = process.argv[2];
const outputPath = process.argv[3];
if (!screenPath || !outputPath) {
  throw new Error('usage: node create-performance-confirm-matrix.mjs SCREEN_RAW OUTPUT_MATRIX');
}
const bytes = await readFile(screenPath);
const report = JSON.parse(bytes);
validatePerformanceMatrix(report.matrix);
if (
  report.matrix.lane !== 'independent-vue-wall-screen' ||
  report.measurementClass !== 'formal local wall evidence subject to host gates' ||
  report.admitted !== true ||
  report.harness?.clean !== true ||
  !report.correctnessEvidence
) {
  throw new Error('confirmation generation requires an admitted clean formal wall screen');
}
for (const run of report.runs) {
  if (
    run.pagingDelta?.pageouts !== 0 ||
    run.pagingDelta?.swapouts !== 0 ||
    run.hostAdmission?.phase !== 'before-child' ||
    run.postHostAdmission?.phase !== 'after-child'
  ) {
    throw new Error(`screen run is not host-eligible: ${run.projectId}/${run.variant}`);
  }
}
const screenRawSha256 = createHash('sha256').update(bytes).digest('hex');
const matrix = createConfirmationMatrixFromScreen(report, screenRawSha256);
await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(matrix, null, 2)}\n`);
console.log(
  JSON.stringify({
    outputPath,
    screenRawSha256,
    cases: matrix.cases.map(({ projectId, selectedScreenWorkerCount, repeats, variants }) => ({
      projectId,
      selectedScreenWorkerCount,
      repeats,
      variants,
    })),
  }),
);
