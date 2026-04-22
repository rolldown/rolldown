import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const onResolved = vi.fn();

export default defineTest({
  config: {
    input: './main.js',
    platform: 'browser',
    plugins: [
      {
        name: 'probe',
        async resolveId(source, importer, options) {
          if (options?.isEntry || !importer) {
            return;
          }
          const resolved = await this.resolve(source, importer, {
            ...options,
            skipSelf: true,
          });
          onResolved(resolved);
          return resolved;
        },
      },
    ],
  },
  afterTest() {
    expect(onResolved).toHaveBeenCalledTimes(1);
    expect(onResolved).toHaveBeenCalledWith(expect.objectContaining({ ignored: true }));
  },
});
