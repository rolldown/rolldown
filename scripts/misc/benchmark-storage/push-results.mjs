// Pushes one benchmark run's results to rolldown/benchmark-results-storage.
//
// Reads the github-action-benchmark data file the action just updated, converts
// its newest "Node Benchmark" entry into Entry-schema JSON Lines (the format
// rolldown/metric's `metric.json` uses — see the storage repo's README), and
// appends them to `benchmark-node-output.jsonl`. Pushes with a pull-rebase retry
// loop, so two concurrent main runs append instead of overwriting each other
// (the previous whole-file copy step silently dropped the losing run).
//
// Usage (CI):    node scripts/misc/benchmark-storage/push-results.mjs
//   env: API_TOKEN_GITHUB — token with push access to the storage repo
// Usage (local): node .../push-results.mjs --data <file> --dry-run <existing-clone-dir>
//   Applies the same changes to <existing-clone-dir> and commits, but never pushes.
//
// Keep the bench→Entry mapping in sync with `scripts/migrate.mjs` in the
// storage repo (it applied the same rules to the historical data).

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const SERIES = 'Node Benchmark';
const STORAGE_REPO = 'github.com/rolldown/benchmark-results-storage.git';
const JSONL_FILE = 'benchmark-node-output.jsonl';
const PEAK_MEMORY_SUFFIX = ' (peak memory)';
const PUSH_RETRIES = 3;

function parseArgs(argv) {
  const args = { data: 'tmp/window.json', dryRun: null };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--data') args.data = argv[++i];
    else if (argv[i] === '--dry-run') args.dryRun = argv[++i];
    else throw new Error(`unknown argument ${argv[i]}`);
  }
  return args;
}

function benchToLine(bench, entry, repoUrl) {
  const value = Number(bench.value);
  if (!Number.isFinite(value)) throw new Error(`non-numeric value ${JSON.stringify(bench)}`);
  let caseName = bench.name;
  let metric = 'production build time';
  let unit = 'ms';
  if (bench.unit === 'bytes' && bench.name.endsWith(PEAK_MEMORY_SUFFIX)) {
    caseName = bench.name.slice(0, -PEAK_MEMORY_SUFFIX.length);
    metric = 'peak memory';
    unit = 'byte';
  } else if (bench.unit !== 'ms / ops') {
    throw new Error(`unmapped unit ${JSON.stringify(bench)}`);
  }
  return JSON.stringify({
    case: caseName,
    metric,
    timestamp: entry.date,
    commit: entry.commit.id,
    unit,
    records: { rolldown: value },
    repoUrl,
  });
}

function git(cwd, ...args) {
  const res = spawnSync('git', args, { cwd, stdio: ['ignore', 'inherit', 'inherit'] });
  return res.status === 0;
}

function gitOrThrow(cwd, ...args) {
  if (!git(cwd, ...args)) throw new Error(`git ${args[0]} failed`);
}

const args = parseArgs(process.argv.slice(2));

const data = JSON.parse(fs.readFileSync(args.data, 'utf8'));
const entries = data.entries?.[SERIES];
if (!entries?.length) throw new Error(`no "${SERIES}" entries in ${args.data}`);
const entry = entries.at(-1);
if (!entry.commit?.id || !entry.date || !Array.isArray(entry.benches)) {
  throw new Error(`unexpected entry shape: ${JSON.stringify(entry).slice(0, 200)}`);
}
const repoUrl = entry.commit.url?.split('/commit/')[0] ?? 'https://github.com/rolldown/rolldown';
const lines = entry.benches.map((bench) => benchToLine(bench, entry, repoUrl));

let workdir = args.dryRun;
if (!workdir) {
  const token = process.env.API_TOKEN_GITHUB;
  if (!token) throw new Error('API_TOKEN_GITHUB is not set');
  workdir = fs.mkdtempSync('benchmark-storage-');
  const url = `https://x-access-token:${token}@${STORAGE_REPO}`;
  const clone = spawnSync('git', ['clone', '--depth', '1', url, workdir], { stdio: 'inherit' });
  if (clone.status !== 0) throw new Error('clone of the storage repo failed');
  gitOrThrow(workdir, 'config', 'user.name', 'github-actions[bot]');
  gitOrThrow(workdir, 'config', 'user.email', 'github-actions[bot]@users.noreply.github.com');
}

const applyChanges = () => {
  // Append this run's lines — unless they are already there (job rerun, or a
  // rebase retry after our own commit survived).
  const jsonlPath = path.join(workdir, JSONL_FILE);
  const existing = fs.existsSync(jsonlPath) ? fs.readFileSync(jsonlPath, 'utf8') : '';
  const lastLine = existing.trimEnd().split('\n').at(-1);
  const tail = lastLine ? JSON.parse(lastLine) : {};
  if (tail.commit === entry.commit.id && tail.timestamp === entry.date) {
    console.log(`lines for ${entry.commit.id.slice(0, 9)} already present, appending nothing`);
    return false;
  }
  fs.appendFileSync(jsonlPath, lines.join('\n') + '\n', 'utf8');
  return true;
};

const appended = applyChanges();
gitOrThrow(workdir, 'add', JSONL_FILE);
const message = `Append results for rolldown/rolldown@${entry.commit.id.slice(0, 9)}`;
if (!git(workdir, 'commit', '-m', message)) {
  console.log('nothing to commit');
  process.exit(0);
}
console.log(`committed: ${message} (${appended ? lines.length : 0} lines appended)`);

if (args.dryRun) {
  console.log('dry run, not pushing');
  process.exit(0);
}

for (let attempt = 1; ; attempt++) {
  if (git(workdir, 'push')) break;
  if (attempt >= PUSH_RETRIES) throw new Error(`push failed after ${PUSH_RETRIES} attempts`);
  console.log(`push rejected, rebasing (attempt ${attempt})`);
  // Rebase our append commit onto whatever the concurrent run pushed. The jsonl
  // append can conflict textually; redo the changes on top instead of merging.
  if (!git(workdir, 'pull', '--rebase')) {
    gitOrThrow(workdir, 'rebase', '--abort');
    gitOrThrow(workdir, 'reset', '--hard', 'origin/main');
    gitOrThrow(workdir, 'pull');
    applyChanges();
    gitOrThrow(workdir, 'add', JSONL_FILE);
    if (!git(workdir, 'commit', '-m', message)) break;
  }
}
console.log('pushed');
