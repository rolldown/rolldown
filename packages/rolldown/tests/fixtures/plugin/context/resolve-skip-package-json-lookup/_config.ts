import nodePath from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const fn = vi.fn();
const target = nodePath.join(import.meta.dirname, 'target.js');

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      {
        name: 'forwarder',
        async resolveId(id, importer) {
          if (id === '@forwarded') {
            // `viteAliasPlugin` returns `this.resolve`'s answer as its own, so the opt-out has
            // to come back out of `this.resolve`.
            const resolved = await this.resolve('@target', importer);
            expect(resolved?.skipPackageJsonLookup).toBe(true);
            fn();
            return resolved;
          }
        },
      },
      {
        name: 'opt-out',
        resolveId(id) {
          if (id === '@target') {
            // The Vite resolver keeps the id bare for `legacyInconsistentCjsInterop`.
            return { id: target, skipPackageJsonLookup: true };
          }
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
