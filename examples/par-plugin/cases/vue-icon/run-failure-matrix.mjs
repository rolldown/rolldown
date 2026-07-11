import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { cpus, platform, release, totalmem, tmpdir } from 'node:os';
import nodePath from 'node:path';

const outputPath = process.argv[2];
const runs = [];
const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const fixtureDirectory = await mkdtemp(nodePath.join(tmpdir(), 'rolldown-parallel-vue-failure-'));
try {
  const source = await readFile(nodePath.join(import.meta.dirname, 'failure/invalid.vue.fixture'));
  await writeFile(nodePath.join(fixtureDirectory, 'invalid.vue'), source);
  for (const variant of ['full-ordinary', 'ordinary', 'worker-1']) {
    const environment = { ...process.env };
    delete environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
    if (variant === 'worker-1') environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS = '1';
    const result = spawnSync(process.execPath, ['./run-failure.mjs', variant, fixtureDirectory], {
      cwd: import.meta.dirname,
      encoding: 'utf8',
      env: environment,
    });
    if (result.status !== 0) {
      throw new Error(`${variant} failure probe exited ${result.status}:\n${result.stderr}`);
    }
    runs.push(
      JSON.parse(
        JSON.stringify(JSON.parse(result.stdout)).replaceAll(
          fixtureDirectory,
          '<vue-failure-fixture>',
        ),
      ),
    );
  }
} finally {
  await rm(fixtureDirectory, { recursive: true, force: true });
}
const report = {
  node: process.version,
  nodeBinary: process.execPath,
  rolldownCommit: git(['rev-parse', 'HEAD']),
  rolldownWorktreeStatus: git(['status', '--short']),
  host: {
    platform: platform(),
    release: release(),
    architecture: process.arch,
    cpuModel: cpus()[0]?.model,
    logicalCpuCount: cpus().length,
    totalMemoryBytes: totalmem(),
  },
  capturedAt: new Date().toISOString(),
  runs,
};
const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (outputPath) await writeFile(outputPath, serialized);
process.stdout.write(serialized);

function git(args) {
  const result = spawnSync('git', ['-C', repositoryRoot, ...args], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ${args.join(' ')} failed`);
  return result.stdout.trim();
}
