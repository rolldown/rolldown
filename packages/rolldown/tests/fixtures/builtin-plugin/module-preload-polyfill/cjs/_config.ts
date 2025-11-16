import { defineTest } from 'rolldown-tests';
import { viteModulePreloadPolyfillPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      format: 'cjs',
    },
    plugins: [viteModulePreloadPolyfillPlugin()],
  },
  async afterTest(output) {
    expect(output.output[0].code.length).toBe(0);
  },
});
