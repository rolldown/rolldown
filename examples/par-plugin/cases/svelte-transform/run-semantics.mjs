import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import nodePath from 'node:path';

if (process.version !== 'v24.18.0') {
  throw new Error(`Svelte semantics probe requires Node.js v24.18.0, got ${process.version}`);
}
const outputPath = process.argv[2];
const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const gitCommit = spawnSync('git', ['-C', repositoryRoot, 'rev-parse', 'HEAD'], {
  encoding: 'utf8',
});
const gitStatus = spawnSync('git', ['-C', repositoryRoot, 'status', '--short'], {
  encoding: 'utf8',
});
const fixtureRoot = await mkdtemp(nodePath.join(tmpdir(), 'rolldown-svelte-semantics-'));
const results = {};

try {
  for (const [scenario, source] of [
    ['warning', '<div onclick={() => {}}>click</div>\n'],
    ['error', '{#if true}<p>broken{/each}\n'],
  ]) {
    const fixtureDirectory = nodePath.join(fixtureRoot, scenario);
    await mkdir(fixtureDirectory);
    const componentPath = nodePath.join(fixtureDirectory, 'Component.svelte');
    const entryPath = nodePath.join(fixtureDirectory, 'entry.js');
    await writeFile(componentPath, source);
    await writeFile(entryPath, "import './Component.svelte';\n");
    results[scenario] = ['ordinary', 'worker-2'].map((variant) => {
      const environment = { ...process.env };
      delete environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
      if (variant === 'worker-2') environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS = '2';
      const child = spawnSync(
        process.execPath,
        [
          nodePath.join(import.meta.dirname, 'run-semantics-case.mjs'),
          JSON.stringify({ variant, fixtureDirectory, entryPath }),
        ],
        { encoding: 'utf8', env: environment },
      );
      if (child.status !== 0) {
        throw new Error(`${scenario}/${variant} process failed:\n${child.stderr}`);
      }
      return JSON.parse(child.stdout);
    });
  }
} finally {
  await rm(fixtureRoot, { recursive: true, force: true });
}

const [ordinaryWarning, workerWarning] = results.warning;
const [ordinaryError, workerError] = results.error;
if (!ordinaryWarning.success || !workerWarning.success) {
  throw new Error('warning semantics fixtures did not build successfully');
}
if (ordinaryError.success || workerError.success) {
  throw new Error('error semantics fixtures unexpectedly built successfully');
}
if (
  ordinaryWarning.outputCodeHash !== workerWarning.outputCodeHash ||
  ordinaryWarning.outputMapHash !== workerWarning.outputMapHash
) {
  throw new Error('warning fixture output differs between ordinary and worker variants');
}

const report = {
  schema: 1,
  timestamp: new Date().toISOString(),
  node: process.version,
  nodeBinary: process.execPath,
  svelteVersion: '5.56.4',
  rolldownCommit: gitCommit.stdout.trim(),
  rolldownWorktreeStatus: gitStatus.stdout.trim(),
  warning: {
    ordinary: ordinaryWarning,
    worker: workerWarning,
    sameLogs: JSON.stringify(ordinaryWarning.logs) === JSON.stringify(workerWarning.logs),
  },
  error: {
    ordinary: ordinaryError,
    worker: workerError,
    sameStructuredError: JSON.stringify(ordinaryError.error) === JSON.stringify(workerError.error),
  },
};
const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (outputPath) {
  await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
  await writeFile(outputPath, serialized);
  console.log(JSON.stringify({ outputPath }));
} else {
  process.stdout.write(serialized);
}
