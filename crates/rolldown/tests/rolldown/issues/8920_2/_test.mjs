import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

// Regression for the fuzz-discovered chunk cycle that followed the fix for
// #8989 (PR #9057). The peel-and-place logic for the runtime module placed
// `__exportAll` into a single `consumer_chunks` element whenever that set
// had exactly one member. When that chunk already had an outbound static
// import to another chunk that also needed helpers, the new helper
// back-edge closed a cycle (entry-2 ↔ node1 here).
//
// After the fix, the runtime is placed in the *dominator* of the consumer
// set — a chunk every other consumer already reaches via forward static
// edges — so helper imports follow the existing DAG. For this fixture the
// dominator is `entry-2.js`, giving a 3-chunk output with no cycle.

const distDir = path.join(import.meta.dirname, 'dist');
const read = (name) => fs.readFileSync(path.join(distDir, name), 'utf8');

const entry0 = read('entry-0.js');
const entry2 = read('entry-2.js');
const node1 = read('node1.js');

// Runtime helpers live in entry-2.js (the dominator); no dedicated
// rolldown-runtime.js chunk is emitted.
assert(entry2.includes('rolldown/runtime.js'), 'entry-2.js should host the runtime module');
assert(
  !fs.existsSync(path.join(distDir, 'rolldown-runtime.js')),
  'no dedicated rolldown-runtime.js chunk should be emitted',
);

// No static cycle between entry-2 and node1: node1 may forward-import from
// entry-2, but entry-2 must not statically import from node1.
assert(
  /import \{[^}]*\} from "\.\/entry-2\.js"/.test(node1),
  'node1.js should statically import from entry-2.js (forward edge)',
);
assert(
  !/^import .* from "\.\/node1\.js"/m.test(entry2),
  'entry-2.js must not statically import from node1.js (would close a cycle)',
);

// entry-0 reaches the runtime through node1 → entry-2.
assert(/from "\.\/node1\.js"/.test(entry0), 'entry-0.js should import from node1.js');
