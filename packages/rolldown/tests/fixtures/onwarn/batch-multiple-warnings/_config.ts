import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

// Guards that when a build emits many warnings they are all delivered to the
// `onwarn` handler. The warnings are batched across the Rust->JS NAPI boundary
// in a single call, so this protects against any loss/reordering introduced by
// the batching.
const fn = vi.fn();
const codes: string[] = [];

export default defineTest({
  sequential: true,
  config: {
    input: './entry.js',
    onwarn(warning) {
      fn();
      codes.push(warning.code!);
    },
    checks: {
      circularDependency: true,
    },
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(3);
    expect(codes).toEqual(['CIRCULAR_DEPENDENCY', 'CIRCULAR_DEPENDENCY', 'CIRCULAR_DEPENDENCY']);
  },
});
