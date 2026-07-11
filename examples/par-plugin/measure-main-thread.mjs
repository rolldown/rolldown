import { createHash } from 'node:crypto';
import { builtinModules } from 'node:module';
import nodePath from 'node:path';
import { monitorEventLoopDelay, performance } from 'node:perf_hooks';
import { setTimeout as delay } from 'node:timers/promises';
import { rolldown } from 'rolldown';
import parallelNoopPlugin from './parallel-noop-plugin/index.js';
import { noopPlugin } from './parallel-noop-plugin/impl.js';

const mode = process.argv[2];
if (mode !== 'ordinary' && mode !== 'worker-1') {
  throw new Error('mode must be ordinary or worker-1');
}

const repoRoot = nodePath.resolve(import.meta.dirname, '../..');
const input = nodePath.join(repoRoot, 'tmp/bench/three10x/entry.js');
const eventLoop = monitorEventLoopDelay({ resolution: 1 });
eventLoop.enable();
await delay(25);
eventLoop.reset();

const cpuStart = process.cpuUsage();
const startedAt = performance.now();
const build = await rolldown({
  input,
  external: builtinModules,
  plugins: [mode === 'ordinary' ? noopPlugin() : parallelNoopPlugin()],
});
const result = await build.generate({ format: 'esm' });
await build.close();
const elapsedMs = performance.now() - startedAt;
const cpu = process.cpuUsage(cpuStart);

await delay(25);
eventLoop.disable();

const chunks = result.output
  .filter((output) => output.type === 'chunk')
  .sort((a, b) => a.fileName.localeCompare(b.fileName));
const hash = createHash('sha256');
let outputBytes = 0;
for (const chunk of chunks) {
  outputBytes += Buffer.byteLength(chunk.code);
  hash.update(chunk.fileName);
  hash.update('\0');
  hash.update(chunk.code);
  hash.update('\0');
}

const fromNanoseconds = (value) => Number(value) / 1e6;
console.log(
  JSON.stringify({
    mode,
    workerCount: mode === 'ordinary' ? 0 : 1,
    elapsedMs,
    cpuUserMs: cpu.user / 1000,
    cpuSystemMs: cpu.system / 1000,
    finalRssBytes: process.memoryUsage.rss(),
    eventLoopDelayMs: {
      min: fromNanoseconds(eventLoop.min),
      mean: fromNanoseconds(eventLoop.mean),
      p50: fromNanoseconds(eventLoop.percentile(50)),
      p95: fromNanoseconds(eventLoop.percentile(95)),
      p99: fromNanoseconds(eventLoop.percentile(99)),
      max: fromNanoseconds(eventLoop.max),
    },
    outputBytes,
    outputHash: hash.digest('hex'),
  }),
);
