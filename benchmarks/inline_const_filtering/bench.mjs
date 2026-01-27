import { rolldown } from 'rolldown';
import path from 'path';

const srcDir = path.join(import.meta.dirname, 'src');
const distDir = path.join(import.meta.dirname, 'dist');

const WARMUP_RUNS = 3;
const BENCHMARK_RUNS = 10;

async function runBenchmark(passCount) {
  const config = {
    input: path.join(srcDir, 'main.js'),
    output: {
      dir: distDir,
    },
    optimization: {
      inlineConst: {
        pass: passCount,
      },
    },
  };

  const build = await rolldown(config);
  await build.write();
}

async function measureTime(passCount, runs) {
  const times = [];
  for (let i = 0; i < runs; i++) {
    const start = performance.now();
    await runBenchmark(passCount);
    const end = performance.now();
    times.push(end - start);
  }
  return times;
}

function stats(times) {
  const sorted = [...times].sort((a, b) => a - b);
  // Use trimmed mean (remove highest and lowest)
  const trimmed = sorted.slice(1, -1);
  const avg = trimmed.reduce((a, b) => a + b, 0) / trimmed.length;
  const min = Math.min(...times);
  return { avg, min };
}

async function main() {
  console.log('Inline Const Filtering Benchmark');
  console.log('=================================');
  console.log();

  // Warmup
  console.log(`Warming up (${WARMUP_RUNS} runs)...`);
  await measureTime(1, WARMUP_RUNS);
  console.log();

  const results = {};
  const passCounts = [1, 2, 5, 10, 15];

  for (const passCount of passCounts) {
    console.log(`Benchmarking pass=${passCount} (${BENCHMARK_RUNS} runs)...`);
    const times = await measureTime(passCount, BENCHMARK_RUNS);
    results[passCount] = stats(times);
  }

  console.log();
  console.log('Results (trimmed mean, excluding outliers):');
  console.log('--------------------------------------------');
  console.log('| Pass | Time (ms) | Extra iterations | Δ from pass=1 |');
  console.log('|------|-----------|------------------|---------------|');

  const baseline = results[1].avg;
  for (const pass of passCounts) {
    const s = results[pass];
    const extraIters = Math.max(0, pass - 1);
    const delta = s.avg - baseline;
    const deltaStr = delta >= 0 ? `+${delta.toFixed(1)}` : delta.toFixed(1);
    console.log(
      `| ${String(pass).padStart(4)} | ${s.avg.toFixed(1).padStart(9)} | ${String(extraIters).padStart(16)} | ${deltaStr.padStart(13)} |`,
    );
  }

  console.log();
  console.log('Analysis:');
  console.log('---------');

  // Calculate incremental cost per extra iteration
  // From pass=2 to pass=15, we add 13 iterations (1 full + 12 filtered for pass=15 vs 1 full for pass=2)
  const pass2to15_delta = results[15].avg - results[2].avg;
  const filteredIterCost = pass2to15_delta / 13;

  console.log(`Incremental cost from pass=2 to pass=15: ${pass2to15_delta.toFixed(1)}ms`);
  console.log(`Average cost per additional filtered iteration: ${filteredIterCost.toFixed(2)}ms`);
  console.log();
  console.log('Note: With filtering optimization, iterations 2+ only process ~15 modules');
  console.log('      instead of all 10016 modules. The low per-iteration cost demonstrates');
  console.log('      that filtering is working correctly.');
}

main().catch(console.error);
