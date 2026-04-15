import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

// Regression for #8989: facade chunk elimination used to add a runtime-helper
// edge from entry2 → the chunk hosting the runtime, while that host chunk
// already had a forward path entry2 → entry3 → node4 → entry2, closing a cycle.
//
// After the fix, the runtime is peeled out of its co-located host and placed
// into a leaf chunk (here, node4.js). Both helper consumers (entry2 + node4)
// must reach helpers via forward-only edges; no chunk that imports helpers
// from another chunk may have a static import edge in the opposite direction.

const distDir = path.join(import.meta.dirname, 'dist');
const read = (name) => fs.readFileSync(path.join(distDir, name), 'utf8');

const entry1 = read('entry1.js');
const entry2 = read('entry2.js');
const entry3 = read('entry3.js');
const node4 = read('node4.js');

// node4 is the leaf runtime host: no static imports from any other chunk.
assert(
  !/^import .* from ".\/(entry|node)/m.test(node4),
  'node4.js must be a leaf chunk (no static imports from sibling chunks)',
);

// node4 hosts the runtime helpers and exports __exportAll for consumers.
assert(node4.includes('rolldown/runtime.js'), 'node4.js should host the runtime module');
assert(
  /export \{[^}]*__exportAll/.test(node4),
  'node4.js should re-export __exportAll for the other consumer chunks',
);

// entry2 needs __exportAll for node2_exports namespace materialization and
// must import it from node4 (the leaf), not the other way around.
assert(
  /import \{[^}]*__exportAll[^}]*\} from "\.\/node4\.js"/.test(entry2),
  'entry2.js should import __exportAll from node4.js',
);
assert(!entry2.includes('from "./entry2.js"'), 'entry2.js should not self-import');

// entry3 statically depends on node4 (re-export of node_4) — this is the
// forward edge. It must NOT have any back-edge to entry2 that would let
// node4 → entry2 close a cycle.
assert(/from "\.\/node4\.js"/.test(entry3), 'entry3.js should statically import from node4.js');

// entry1 transitively reaches entry2 and entry3, both of which are forward
// edges away from node4 — so the static graph must remain acyclic.
assert(/from "\.\/entry2\.js"/.test(entry1), 'entry1.js should import from entry2.js');
assert(/from "\.\/entry3\.js"/.test(entry1), 'entry1.js should import from entry3.js');
