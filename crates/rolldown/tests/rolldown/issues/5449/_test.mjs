import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

// Regression test for #5449. Before #7022 reserved global names ahead of deconflicting, merging
// the dynamic imports into the entry chunk hoisted
// `let process = require("process"); process = __toESM(process)` for process-importer.js, shadowing
// the global `process` for global-user.js, whose `process.on(...)` then threw
// `Cannot set property _eventsCount of #<process> which has only a getter` on the read-only copy.
const require = createRequire(import.meta.url);
const { done } = require('./dist/main.js');
await done;

assert.ok(globalThis.__globalProcessOk, 'global-user.js must reach the real global `process`');
assert.equal(globalThis.__importedProcessPid, process.pid);
