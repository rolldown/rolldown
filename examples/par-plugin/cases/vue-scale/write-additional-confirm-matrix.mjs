import { mkdir, readFile, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';
import { assertLocalExecution } from './provenance.mjs';

assertLocalExecution();
const summaryPath = process.argv[2];
const outputPath = process.argv[3];
if (!summaryPath || !outputPath) {
  throw new Error('expected <confirmation-summary.json> <additional-matrix.json>');
}
const summary = JSON.parse(await readFile(summaryPath, 'utf8'));
if (!summary.additionalConfirmationMatrix) {
  throw new Error('summary does not request another confirmation iteration');
}
await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(summary.additionalConfirmationMatrix, null, 2)}\n`);
console.log(
  JSON.stringify({
    outputPath,
    scales: summary.additionalConfirmationMatrix.cases.map(({ componentCount }) => componentCount),
  }),
);
