import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const onLogFn = vi.fn();

export default defineTest({
  sequential: true,
  config: {
    output: {
      banner: () => '#!/usr/bin/env node',
    },
    onLog(level, log) {
      expect(level).toBe('warn');
      expect(log.code).toBe('DUPLICATE_SHEBANG');
      expect(log.message).toContain('banner');
      onLogFn();
    },
  },
  afterTest: () => {
    // Should have a warning about duplicate shebang
    expect(onLogFn).toHaveBeenCalledTimes(1);
  },
});
