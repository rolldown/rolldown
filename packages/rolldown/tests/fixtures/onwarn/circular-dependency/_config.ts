import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const fn = vi.fn();

export default defineTest({
  sequential: true,
  config: {
    onwarn(warning) {
      fn();
      expect(warning.code).toBe('CIRCULAR_DEPENDENCY');
    },
    checks: {
      circularDependency: true,
    },
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
