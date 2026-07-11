import { spawnSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { cpus, platform, release, totalmem } from 'node:os';
import nodePath from 'node:path';

const outputPath = process.argv[2];
const startedAt = new Date().toISOString();
const repositoryRoot = nodePath.resolve(import.meta.dirname, '../..');
const gitCommit = spawnSync('git', ['-C', repositoryRoot, 'rev-parse', 'HEAD'], {
  encoding: 'utf8',
});
if (gitCommit.status !== 0) throw new Error('failed to identify the Rolldown commit');
const gitStatus = spawnSync('git', ['-C', repositoryRoot, 'status', '--short'], {
  encoding: 'utf8',
});
if (gitStatus.status !== 0) throw new Error('failed to inspect the Rolldown worktree');

const runs = [];
for (const mode of ['ordinary', 'worker-1']) {
  for (let index = 0; index < 5; index++) {
    const result = spawnSync(
      '/usr/bin/time',
      ['-l', process.execPath, './measure-main-thread.mjs', mode],
      {
        cwd: import.meta.dirname,
        encoding: 'utf8',
        env: {
          ...process.env,
          ...(mode === 'worker-1' ? { ROLLDOWN_PARALLEL_PLUGIN_WORKERS: '1' } : {}),
        },
      },
    );
    if (result.status !== 0) {
      process.stderr.write(result.stderr);
      process.exit(result.status ?? 1);
    }
    const peakRssMatch = result.stderr.match(/(\d+)\s+maximum resident set size/);
    if (!peakRssMatch) {
      throw new Error('failed to read maximum resident set size from /usr/bin/time -l');
    }
    runs.push({ index, peakRssBytes: Number(peakRssMatch[1]), ...JSON.parse(result.stdout) });
  }
}

const report = JSON.stringify(
  {
    node: process.version,
    nodeBinary: process.execPath,
    rolldownCommit: gitCommit.stdout.trim(),
    rolldownWorktreeStatus: gitStatus.stdout.trim(),
    host: {
      platform: platform(),
      release: release(),
      architecture: process.arch,
      cpuModel: cpus()[0]?.model,
      logicalCpuCount: cpus().length,
      totalMemoryBytes: totalmem(),
    },
    startedAt,
    finishedAt: new Date().toISOString(),
    runs,
  },
  null,
  2,
);

if (outputPath) {
  writeFileSync(outputPath, `${report}\n`);
}
console.log(report);
