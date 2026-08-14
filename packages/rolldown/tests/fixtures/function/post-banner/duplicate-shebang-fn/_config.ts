import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const onLogFn = vi.fn();

export default defineTest({
  sequential: true,
  config: {
    output: {
      postBanner: () => '#!/usr/bin/env bun\n/* version 1.0.0 */',
    },
    onLog(level, log) {
      expect(level).toBe('warn');
      expect(log.code).toBe('DUPLICATE_SHEBANG');
      expect(log.message).toContain('postBanner');
      onLogFn();
    },
  },
  afterTest: () => {
    expect(onLogFn).toHaveBeenCalledTimes(1);
  },
});
