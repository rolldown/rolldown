import { defineTest } from 'rolldown-tests';
import { isWasiTest } from 'rolldown-tests/utils';
import { expect, vi } from 'vitest';

const fn = vi.fn();

export default defineTest({
  // Under the wasm binding the error arrives with a `wasm://` stack and none of the `Caused by:` chain asserted below.
  skip: isWasiTest,
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart(this) {
          await this.resolve('./sub.js', undefined, { skipSelf: false });
          fn();
        },
        async resolveId(id) {
          if (id === './sub.js') {
            throw new Error('my error');
          }
          return null;
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).not.toHaveBeenCalled();
  },
  catchError(err: any) {
    expect(err).toBeInstanceOf(Error);
    expect(err.message).toContain('Errored while resolving "./sub.js" in `this.resolve`.');
    expect(err.message).toContain('Caused by:');
    expect(err.message).toContain('Error: my error');
  },
});
