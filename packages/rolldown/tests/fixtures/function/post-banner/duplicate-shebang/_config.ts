import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const postBanner = '#!/usr/bin/env bun\n/* version 1.0.0 */';
const onLogFn = vi.fn();

export default defineTest({
  sequential: true,
  config: {
    output: {
      postBanner,
    },
    onLog(level, log) {
      expect(level).toBe('warn');
      expect(log.code).toBe('DUPLICATE_SHEBANG');
      onLogFn();
    },
  },
  afterTest: (output) => {
    // Should start with the original shebang, not the postBanner's shebang
    expect(output.output[0].code.startsWith('#!/usr/bin/env node')).toBe(true);
    // Should have the postBanner content after the original shebang
    expect(output.output[0].code.includes('#!/usr/bin/env bun')).toBe(true);
    expect(output.output[0].code.includes('/* version 1.0.0 */')).toBe(true);
    // Should have a warning about duplicate shebang
    expect(onLogFn).toHaveBeenCalledTimes(1);
  },
});
