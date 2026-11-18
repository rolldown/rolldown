import path from 'node:path';
import type { PluginContext } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

let fooHookCalls = 0;
let loadFromBuildStart: PluginContext['load'];

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart() {
          loadFromBuildStart = this.load.bind(this);
          await expect(
            this.load({
              id: path.join(__dirname, 'foo.js'),
            }),
          ).rejects.toThrow('`this.load` is not supported in the `buildStart` hook.');
        },
        async load(id) {
          if (id.endsWith('main.js')) {
            const moduleInfo = await loadFromBuildStart({
              id: path.join(__dirname, 'foo.js'),
              moduleSideEffects: false,
            });
            expect(moduleInfo.code!.includes('foo')).toBe(true);

            const resolved = await this.resolve('./dir/index.js', id);
            expect(resolved).toBeDefined();
            await this.load(resolved!);
          }
          if (id.endsWith('foo.js')) {
            fooHookCalls++;
          }
        },
        async transform(code, id) {
          if (id.endsWith('main.js')) {
            const moduleInfo = await this.load({
              id: path.join(__dirname, 'foo.js'),
            });
            expect(moduleInfo.code!.includes('foo')).toBe(true);
            // should reusing exiting modules
            expect(fooHookCalls).toBe(1);
          }
        },
      },
    ],
  },
  beforeTest: () => {
    fooHookCalls = 0;
  },
  afterTest: async (_output) => {
    // @ts-ignore
    await import('./dist/main');
  },
});
