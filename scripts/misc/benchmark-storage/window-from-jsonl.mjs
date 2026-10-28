// Reconstructs a small github-action-benchmark data file from the tail of the
// storage repo's Entry-schema JSON Lines.
//
// Not wired into the workflow yet: while the migration's dual-write window is
// open, the action keeps operating on the full nested JSON. At cutover the
// workflow feeds the action this windowed reconstruction instead (the action
// only compares against the previous entry, so a small window is enough), and
// the 17 MB JSON stops being written.
//
// Usage: node .../window-from-jsonl.mjs --jsonl <file> --out <file> [--window 50]

import fs from 'node:fs';

const SERIES = 'Node Benchmark';
const PEAK_MEMORY_METRIC = 'peak memory';

const args = { jsonl: null, out: null, window: 50 };
const argv = process.argv.slice(2);
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === '--jsonl') args.jsonl = argv[++i];
  else if (argv[i] === '--out') args.out = argv[++i];
  else if (argv[i] === '--window') args.window = Number(argv[++i]);
  else throw new Error(`unknown argument ${argv[i]}`);
}
if (!args.jsonl || !args.out || !Number.isInteger(args.window) || args.window < 1) {
  throw new Error('required: --jsonl <file> --out <file> [--window <n>]');
}

function lineToBench(line) {
  if (line.metric === PEAK_MEMORY_METRIC) {
    return { name: `${line.case} (peak memory)`, value: line.records.rolldown, unit: 'bytes' };
  }
  return { name: line.case, value: line.records.rolldown, unit: 'ms / ops' };
}

// Lines are append-ordered and each run's lines are contiguous (one commit per
// run), so runs split wherever (commit, timestamp) changes.
const lines = fs
  .readFileSync(args.jsonl, 'utf8')
  .split('\n')
  .filter(Boolean)
  .map((l) => JSON.parse(l));
const runs = [];
for (const line of lines) {
  const current = runs.at(-1);
  if (!current || current.commit.id !== line.commit || current.date !== line.timestamp) {
    runs.push({
      commit: {
        // The lines keep only what the action's summary renders (id, url); the
        // remaining fields exist so the data file stays shape-compatible.
        author: { name: '', username: '' },
        committer: { name: '', username: '' },
        id: line.commit,
        message: '',
        timestamp: new Date(line.timestamp).toISOString(),
        url: `${line.repoUrl}/commit/${line.commit}`,
      },
      date: line.timestamp,
      tool: 'customSmallerIsBetter',
      benches: [],
    });
  }
  runs.at(-1).benches.push(lineToBench(line));
}

const windowed = runs.slice(-args.window);
const data = {
  lastUpdate: windowed.at(-1)?.date ?? 0,
  repoUrl: lines.at(-1)?.repoUrl ?? 'https://github.com/rolldown/rolldown',
  entries: { [SERIES]: windowed },
};
fs.writeFileSync(args.out, JSON.stringify(data), 'utf8');
console.log(`${args.out}: ${windowed.length} runs (of ${runs.length} total) from ${args.jsonl}`);
