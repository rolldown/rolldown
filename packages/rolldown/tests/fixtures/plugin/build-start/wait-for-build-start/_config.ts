import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const buildStartFn = vi.fn();
const buildStartFn2 = vi.fn();
const sleepAsync = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      {
        name: 'test-plugin-1',
        async buildStart() {
          await sleepAsync(100);
          buildStartFn();
        },
        transform(_, id) {
          // Skip virtual modules (like \0rolldown/runtime.js)
          if (id.startsWith('\0')) {
            return;
          }
          expect(buildStartFn).toHaveBeenCalledTimes(1);
          expect(buildStartFn2).toHaveBeenCalledTimes(1);
        },
      },
      {
        name: 'test-plugin-2',
        async buildStart(config) {
          await sleepAsync(100);
          buildStartFn2();
        },
      },
    ],
  },
});
