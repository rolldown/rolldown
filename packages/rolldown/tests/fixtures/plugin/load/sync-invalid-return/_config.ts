import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// A malformed synchronous `load` result fails with the field error, as an async one does.
export default defineTest({
  config: {
    plugins: [
      // @ts-expect-error - `code` is intentionally a number to trigger the error
      {
        name: 'test-plugin',
        load(id) {
          if (id.endsWith('main.js')) {
            return { code: 1 };
          }
        },
      },
    ],
  },
  catchError(e: any) {
    expect(e.message).toContain('into rust type `String` on BindingHookLoadOutput.code');
  },
});
