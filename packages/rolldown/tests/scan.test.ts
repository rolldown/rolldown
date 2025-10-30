import { scan } from 'rolldown/experimental';
import { describe, expect, test } from 'vitest';

describe('experimental_scan', () => {
  test('call options hook', async () => {
    expect.assertions(1);

    await scan({
      input: 'virtual',
      plugins: [
        {
          name: 'test',
          options(opts) {
            expect(opts).toBeTruthy();
          },
          resolveId(id) {
            if (id === 'virtual') return '\0' + id;
          },
          load(id) {
            if (id === '\0virtual') return '';
          },
        },
      ],
    });
  });
});
