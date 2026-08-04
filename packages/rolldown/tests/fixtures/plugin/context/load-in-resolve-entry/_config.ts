import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

let entryId: string | undefined;

export default defineTest({
  config: {
    plugins: [
      {
        name: 'load-in-resolve',
        async resolveId(source, importer, options) {
          const resolved = await this.resolve(source, importer, {
            ...options,
            skipSelf: true,
          });
          if (resolved && !resolved.external) {
            if (options.isEntry) {
              entryId = resolved.id;
            }
            const moduleInfo = await this.load(resolved);
            expect(moduleInfo.code).not.toBeNull();
          }
          return resolved;
        },
        buildEnd() {
          expect(this.getModuleInfo(entryId!)?.isEntry).toBe(true);
        },
      },
    ],
  },
});
