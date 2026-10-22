import { build, rolldown } from 'rolldown';
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

const input = './fixtures/tree-shake/inline-dynamic-imports/main.js';
const cwd = import.meta.dirname + '/..';

test('outputs converted concurrently do not interleave their warnings', async () => {
  await build([
    {
      input,
      cwd,
      write: false,
      output: { legalComments: 'inline', advancedChunks: { groups: [] } },
    },
    { input, cwd, write: false, output: { inlineDynamicImports: true } },
  ]);
  expect(consoleSpy.mock.calls.map(([message]) => message)).toEqual([
    '`legalComments` option is deprecated, please use `comments.legal` instead.',
    '`advancedChunks` option is deprecated, please use `codeSplitting` instead.',
    '`inlineDynamicImports` option is deprecated, please use `codeSplitting: false` instead.',
  ]);
});

test('a warning prints before an `onLog` warning of the same output', async () => {
  const messages: unknown[] = [];
  consoleSpy.mockImplementation((message) => messages.push(message));
  const bundle = await rolldown({
    input,
    cwd,
    onLog(level, log) {
      messages.push(`${level}: ${log.code}`);
    },
  });
  await bundle.generate({ advancedChunks: { groups: [{ name: () => 'shared' }] } });
  expect(messages).toEqual([
    '`advancedChunks` option is deprecated, please use `codeSplitting` instead.',
    'warn: MISSING_CODE_SPLITTING_GROUP_DEBUG_NAME',
  ]);
});

test('a warning prints before an invalid configuration error', async () => {
  const bundle = await rolldown({ input, cwd });
  await expect(
    bundle.generate({
      codeSplitting: false,
      inlineDynamicImports: true,
      manualChunks: () => 'vendor',
    }),
  ).rejects.toThrow(
    'Invalid configuration: "output.manualChunks" cannot be used when "output.codeSplitting" is set to false.',
  );
  expect(consoleSpy).toHaveBeenCalledWith(
    '`inlineDynamicImports` option is ignored because `codeSplitting: false` is set.',
  );
});
