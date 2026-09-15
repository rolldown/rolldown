import { stripAnsi } from 'consola/utils';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    // @ts-expect-error - this is intentionally wrong to trigger the error
    external: (source: string) => (source.startsWith('external') ? 1 : 0),
  },
  catchError(err: any) {
    expect(stripAnsi(err.toString())).toContain(
      'external option: The function returned `number`, but expected `boolean`.',
    );
  },
});
