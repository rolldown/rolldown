import { defineTest } from 'rolldown-tests';
import { viteAliasPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      viteAliasPlugin({
        entries: [{
          find: {} as any,  // Invalid - empty object instead of string or regex
          replacement: ''
        }]
      })
    ]
  },
  async catchError(error) {
    // Should fail with a proper error message
    expect(error).toBeDefined();
    expect((error as Error)?.message).toMatch(/Failed to convert builtin plugin/);
    expect((error as Error)?.message).toMatch(/ViteAlias/);
  }
});
