import { rolldown } from 'rolldown';
import { expect, test, vi, beforeEach, afterEach } from 'vitest';
import type { MockInstance } from 'vitest';

let consoleSpy: MockInstance<typeof console.log>;

beforeEach(() => {
  consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
});

afterEach(() => {
  consoleSpy.mockRestore();
});

test('legalComments shows deprecation warning', async () => {
  const bundle = await rolldown({
    input: './fixtures/tree-shake/inline-dynamic-imports/main.js',
    cwd: import.meta.dirname + '/..',
  });
  await bundle.generate({
    legalComments: 'inline',
  });
  expect(consoleSpy).toHaveBeenCalledWith(
    '`legalComments` option is deprecated, please use `comments.legal` instead.',
  );
});
