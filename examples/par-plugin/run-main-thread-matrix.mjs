import { spawnSync } from 'node:child_process';

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

console.log(JSON.stringify({ runs }, null, 2));
