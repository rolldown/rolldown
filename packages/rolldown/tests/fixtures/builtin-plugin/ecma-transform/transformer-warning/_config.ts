import type { RolldownLog } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { viteTransformPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

const warnings: RolldownLog[] = [];

export default defineTest({
  config: {
    input: './main.ts',
    plugins: [viteTransformPlugin({ root: __dirname, transformOptions: { target: 'es2019' } })],
    onLog(level, log) {
      if (level === 'warn' && log.code === 'TOLERATED_TRANSFORM') {
        warnings.push(log);
      }
    },
  },
  afterTest() {
    // Transformer warnings are reported as warnings and must not fail the build.
    expect(warnings).toHaveLength(1);
    expect(warnings[0].plugin).toBe('builtin:vite-transform');
    expect(warnings[0].message).toContain('Big integer literals are not available');
  },
});
