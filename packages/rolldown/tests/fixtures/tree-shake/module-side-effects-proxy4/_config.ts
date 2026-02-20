import path from 'node:path';
import type { OutputChunk as RolldownOutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  retry: 3, // FIXME: retry added to mitigate flakiness, see https://github.com/rolldown/rolldown/pull/8397
  config: {
    plugins: [
      {
        name: 'loader',
        resolveId(id, importer) {
          if (importer?.includes('foo.js') && id === 'virtual') {
            return { id: '\0virtual', moduleSideEffects: false };
          }
        },
        load(id) {
          if (id === '\0virtual') {
            return 'console.log("sideeffects")';
          }
        },
        async transform(code, id) {
          if (id.includes('bar.js')) {
            const fooPath = path.resolve(import.meta.dirname, 'foo.js');
            const resolved = await this.resolve('virtual', fooPath, { skipSelf: false });
            await this.load({ id: resolved!.id }); // ensure moduleInfo for `virtual` exists
            const moduleInfo = this.getModuleInfo(resolved!.id);
            moduleInfo!.moduleSideEffects = true;
          }
          return {
            code,
          };
        },
      },
    ],
  },
  afterTest: (output) => {
    output.output
      .filter((chunk): chunk is RolldownOutputChunk => chunk.type === 'chunk')
      .forEach((chunk) => {
        expect(chunk.code).toContain('sideeffects');
      });
  },
});
