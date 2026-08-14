import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Regression test for the same class of bug fixed by Rollup PR #6362.
//
// `build_module_groups` (Rust side) used to iterate modules in `ModuleIdx`
// allocation order — the order in which their parents finished
// `resolveId`/`load`/`transform`. Under parallel I/O that order is
// non-deterministic, so a stateful `name` function produced different chunk
// assignments across runs.
//
// The fix sorts modules by `stable_id` before invoking the user function.
// This test asserts the call order is alphabetical (by stable_id) — not the
// source-import order, not the `ModuleIdx` allocation order — so any
// regression to a different iteration order will trip it.

const calls: string[] = [];

export default defineTest({
  config: {
    input: path.join(__dirname, 'entry.js'),
    cwd: __dirname,
    output: {
      codeSplitting: {
        groups: [
          {
            name(id) {
              calls.push(id);
              // Return undefined: skip grouping, we only care about call order.
              return undefined;
            },
          },
        ],
      },
    },
  },
  afterTest() {
    const fixtureCalls = calls
      .filter((id) => id.includes('groups-name-fn-call-order'))
      .map((id) => path.basename(id));

    expect(fixtureCalls).toEqual(['a.js', 'entry.js', 'm.js', 'z.js']);
  },
});
