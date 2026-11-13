import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// The adapter passes `attributes` through, so the getter throws while the native side converts the
// synchronous result. The build fails with that exception, as it does for an async hook.
export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        resolveId(id) {
          if (id === 'foo') {
            return {
              id,
              attributes: {
                get type(): string {
                  throw new Error('attributes getter throws');
                },
              },
            };
          }
        },
        load(id) {
          if (id === 'foo') {
            return '';
          }
        },
      },
    ],
  },
  catchError(e: any) {
    expect(e.message).toContain('attributes getter throws');
  },
});
