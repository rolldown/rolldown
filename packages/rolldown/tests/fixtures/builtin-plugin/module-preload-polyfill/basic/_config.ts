import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteModulePreloadPolyfillPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    plugins: [viteModulePreloadPolyfillPlugin()],
  },
  async afterTest(output) {
    await expect(output.output[0].code).toMatchFileSnapshot(
      path.resolve(import.meta.dirname, 'main.js.snap'),
    );
  },
});
