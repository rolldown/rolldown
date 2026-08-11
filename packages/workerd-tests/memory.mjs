import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const repoRoot = path.resolve(import.meta.dirname, '../..');
const args = new Map(
  process.argv
    .slice(2)
    .map((arg) => arg.split('=', 2))
    .filter((entry) => entry.length === 2),
);
const concurrency = Number(args.get('--concurrency') ?? 2);
const rounds = Number(args.get('--rounds') ?? 3);
const maxRssGrowthMb = Number(args.get('--max-rss-growth-mb') ?? Number.POSITIVE_INFINITY);
const loaderPath = path.resolve(
  repoRoot,
  args.get('--loader') ?? 'packages/rolldown/src/rolldown-binding.wasip1-deferred.js',
);
const wasmPath = path.resolve(
  repoRoot,
  args.get('--wasm') ?? 'packages/rolldown/src/rolldown-binding.wasm32-wasip1.wasm',
);

if (!Number.isInteger(concurrency) || concurrency < 1) {
  throw new TypeError('--concurrency must be a positive integer');
}
if (!Number.isInteger(rounds) || rounds < 1) {
  throw new TypeError('--rounds must be a positive integer');
}
if (Number.isNaN(maxRssGrowthMb) || maxRssGrowthMb < 0) {
  throw new TypeError('--max-rss-growth-mb must be a non-negative number');
}

const loader = await import(pathToFileURL(loaderPath).href);
const instantiate = loader.createInstance ?? loader.instantiate;
const getStats = loader.getDeferredRuntimeStats ?? loader.getWorkerdRuntimeStats;
if (typeof instantiate !== 'function' || typeof getStats !== 'function') {
  throw new TypeError(`Unsupported managed workerd loader: ${loaderPath}`);
}

const wasmModule = await WebAssembly.compile(await readFile(wasmPath));
const samples = [];
const baseline = process.memoryUsage();

for (let round = 1; round <= rounds; round += 1) {
  const before = process.memoryUsage();
  const instances = await Promise.all(
    Array.from({ length: concurrency }, () => instantiate(wasmModule)),
  );
  const memoryObjects = new Set(instances.map((instance) => instance.memory));
  if (memoryObjects.size !== concurrency) {
    throw new Error('Concurrent workerd instances unexpectedly share memory');
  }

  const active = process.memoryUsage();
  const memoryBytes = instances.map((instance) => instance.memoryBytes);
  for (const instance of instances) {
    instance.dispose();
    instance.dispose();
  }
  const disposed = process.memoryUsage();
  globalThis.gc?.();
  await new Promise((resolve) => setTimeout(resolve, 0));
  globalThis.gc?.();
  const afterGc = process.memoryUsage();
  const stats = getStats();
  if (stats.liveInstances !== 0) {
    throw new Error(`Managed workerd instances leaked after round ${round}`);
  }

  samples.push({
    round,
    concurrency,
    memoryBytes,
    before,
    active,
    disposed,
    afterGc,
    rssGrowthFromBaseline: afterGc.rss - baseline.rss,
    stats,
  });
}

const peakRssGrowth = Math.max(...samples.map((sample) => sample.rssGrowthFromBaseline));
if (peakRssGrowth > maxRssGrowthMb * 1024 * 1024) {
  throw new Error(
    `Disposed workerd instances grew RSS by ${Math.ceil(peakRssGrowth / 1024 / 1024)} MiB; ` +
      `limit is ${maxRssGrowthMb} MiB`,
  );
}

console.log(
  JSON.stringify(
    {
      loader: path.relative(repoRoot, loaderPath),
      wasm: path.relative(repoRoot, wasmPath),
      note: 'process.memoryUsage() is a local regression signal. Validate committed memory with Workers DevTools and production memory metrics.',
      baseline,
      peakRssGrowth,
      samples,
    },
    null,
    2,
  ),
);
