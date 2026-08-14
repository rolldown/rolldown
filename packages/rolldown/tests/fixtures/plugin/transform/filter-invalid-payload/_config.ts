// Test that passing invalid filter payload types produces a clean error
// instead of panicking. This simulates a user bypassing TypeScript type
// checking (e.g., plain JS, @ts-ignore) and passing a number where a
// string or RegExp is expected for an `id` filter.
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'bad-filter-plugin',
        transform: {
          filter: {
            id: {
              // This should be a string or RegExp, but we intentionally pass a number to trigger the error handling
              include: [123 as any],
            },
          },
          handler() {},
        },
      },
    ],
  },
  catchError(e: any) {
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toContain('expected a string or regex payload, but got Number(123)');
    expect(e.message).toContain('bad-filter-plugin');
  },
});
