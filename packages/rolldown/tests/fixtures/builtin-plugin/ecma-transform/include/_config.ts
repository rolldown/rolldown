import { defineTest } from 'rolldown-tests';
import { viteTransformPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

let transformed: string[] = [];

// The test is valid, since we process none js by default
// The only thing we need to track file that has `moduleType` as `js`
export default defineTest({
  config: {
    input: './main.ts',
    plugins: [
      viteTransformPlugin({
        root: __dirname,
        include: ['**/node_modules/**'],
      }),
      {
        name: 'test',
        transform(_, id, meta) {
          // Skip virtual modules (like \0rolldown/runtime.js)
          if (id.startsWith('\0')) {
            return null;
          }
          if (meta.moduleType === 'js') {
            transformed.push(id);
          }
          return null;
        },
      },
    ],
  },
  async afterTest() {
    expect(transformed.length).toBe(1);
    expect(transformed.splice(0)[0].includes('node_modules')).toBeTruthy();
  },
});
