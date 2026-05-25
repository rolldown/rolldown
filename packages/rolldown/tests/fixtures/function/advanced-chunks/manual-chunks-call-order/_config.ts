import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Regression test for the same class of bug fixed by Rollup PR #6362.
//
// `output.manualChunks` is a deprecated alias for
// `output.codeSplitting.groups[].name`; both lower to the same Rust-side
// `build_module_groups` iteration. Asserts the legacy alias also sees
// modules in `stable_id` (alphabetical) order — not source-import order,
// not `ModuleIdx` allocation order. See sibling `groups-name-fn-call-order`.

const calls: string[] = [];

export default defineTest({
  config: {
    input: path.join(__dirname, 'entry.js'),
    cwd: __dirname,
    output: {
      manualChunks(id) {
        calls.push(id);
        return null;
      },
    },
  },
  afterTest() {
    // Filter to the fixture's own files (drop the rolldown runtime and any
    // tooling-injected modules) and normalize path separators.
    const fixtureCalls = calls
      .filter((id) => id.includes('manual-chunks-call-order'))
      .map((id) => path.basename(id));

    expect(fixtureCalls).toEqual(['a.js', 'entry.js', 'm.js', 'z.js']);
  },
});
