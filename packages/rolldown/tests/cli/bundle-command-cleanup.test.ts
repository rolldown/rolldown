import { afterEach, expect, test, vi } from 'vitest';

const { asyncDispose, generate, rolldown } = vi.hoisted(() => ({
  asyncDispose: vi.fn(),
  generate: vi.fn(),
  rolldown: vi.fn(),
}));

vi.mock('@src/api/rolldown', () => ({ rolldown }));

import { bundleWithCliOptions } from '@src/cli/commands/bundle';

afterEach(() => {
  process.exitCode = undefined;
  asyncDispose.mockReset();
  generate.mockReset();
  rolldown.mockReset();
});

test('no-output CLI failure disposes the build before returning', async () => {
  generate.mockResolvedValue({ output: [] });
  rolldown.mockResolvedValue({
    [Symbol.asyncDispose]: asyncDispose,
    generate,
  });

  await bundleWithCliOptions({
    input: { input: 'entry.js' },
    output: {},
    watch: false,
  } as never);

  expect(process.exitCode).toBe(1);
  expect(asyncDispose).toHaveBeenCalledOnce();
});
