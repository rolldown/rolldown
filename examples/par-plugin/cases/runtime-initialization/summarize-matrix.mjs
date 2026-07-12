import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import nodePath from 'node:path';
import {
  assertArtifactUnchanged,
  assertDistinctArtifactPaths,
  writeArtifactAtomically,
} from './artifact-io.mjs';
import {
  ATTRIBUTION_RUNTIME,
  captureInitializationHarnessProvenance,
  inspectAttributionRuntime,
} from './provenance.mjs';
import { summarizeInitializationReport } from './summary-core.mjs';

const [inputPath, outputPath] = process.argv.slice(2);
if (!inputPath) throw new Error('expected <formal-raw.json> [summary.json]');
await assertDistinctArtifactPaths(inputPath, outputPath);
const rawBytes = await readFile(inputPath);
const report = JSON.parse(rawBytes);
const currentHarness = await captureInitializationHarnessProvenance({ requireClean: true });
if (
  currentHarness.worktree.commit !== report.harnessProvenance?.worktree?.commit ||
  currentHarness.sourceManifest.aggregateSha256 !==
    report.harnessProvenance?.sourceManifest?.aggregateSha256
) {
  throw new Error('formal initialization report does not match the current clean harness');
}
const currentRuntime = await inspectAttributionRuntime(
  report.runtimeProvenance?.packageRoot,
  ATTRIBUTION_RUNTIME,
);
if (JSON.stringify(currentRuntime) !== JSON.stringify(report.runtimeProvenance)) {
  throw new Error('formal initialization report does not match the current attribution runtime');
}
const summary = summarizeInitializationReport(report, {
  rawArtifact: {
    path: outputPath
      ? nodePath
          .relative(nodePath.dirname(nodePath.resolve(outputPath)), nodePath.resolve(inputPath))
          .split(nodePath.sep)
          .join('/')
      : nodePath.basename(inputPath),
    bytes: rawBytes.byteLength,
    sha256: createHash('sha256').update(rawBytes).digest('hex'),
  },
});
const serialized = `${JSON.stringify(summary, null, 2)}\n`;
if (outputPath) {
  await assertArtifactUnchanged(inputPath, rawBytes);
  await writeArtifactAtomically(outputPath, serialized);
  console.log(JSON.stringify({ outputPath, cases: summary.cases.length }));
} else {
  process.stdout.write(serialized);
}
