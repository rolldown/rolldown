import { rolldown } from 'rolldown';
import { expect, test, vi } from 'vitest';

test('validate input option', async () => {
  const consoleSpy = vi.spyOn(console, 'warn');
  await rolldown({
    // @ts-ignore invalid value
    input: 1,
    cwd: import.meta.dirname,
    // @ts-ignore invalid key
    foo: 'bar',
    resolve: {
      // @ts-ignore nested invalid key
      foo: 'bar',
    },
    watch: {
      // @ts-ignore
      chokidar: {},
    },
    experimental: {
      devMode: {},
    },
  });
  expect(consoleSpy).toHaveBeenCalledWith(
    `\x1b[33mWarning: Invalid input options (4 issues found)\n- For the "input". Invalid type: Expected (string | Array | Object) but received 1. \n- For the "resolve.foo". Invalid key: Expected never but received "foo". \n- For the "watch.chokidar". The "watch.chokidar" option is deprecated, please use "watch.watcher" instead of it. \n- For the "foo". Invalid key: Expected never but received "foo". \x1b[0m`,
  );
});

test('validate output option', async () => {
  const consoleSpy = vi.spyOn(console, 'warn');
  const bundle = await rolldown({
    input: './build-api/main.js',
    cwd: import.meta.dirname,
  });
  await bundle.write({
    // @ts-ignore  invalid key
    foo: 'bar',
    hoistTransitiveImports: false,
  });
  expect(consoleSpy).toHaveBeenCalledWith(
    `\x1b[33mWarning: Invalid output options (1 issue found)\n- For the "foo". Invalid key: Expected never but received "foo". \x1b[0m`,
  );
});

test('requires RegExp for mangleProps include', async () => {
  const consoleSpy = vi.spyOn(console, 'warn');
  const bundle = await rolldown({
    input: './build-api/main.js',
    cwd: import.meta.dirname,
  });
  await expect(
    bundle.generate({
      minify: {
        mangleProps: {
          // @ts-ignore invalid value
          include: '^_',
        },
      },
    }),
  ).rejects.toThrow();
  expect(consoleSpy).toHaveBeenCalledWith(
    expect.stringContaining('Invalid type: Expected RegExp but received "^_"'),
  );
});

test('give a warning for hoistTransitiveImports: true', async () => {
  const consoleSpy = vi.spyOn(console, 'warn');
  const bundle = await rolldown({
    input: './build-api/main.js',
    cwd: import.meta.dirname,
  });
  await bundle.write({
    // @ts-ignore  invalid value
    hoistTransitiveImports: true,
  });
  expect(consoleSpy).toHaveBeenCalledWith(
    `\x1b[33mWarning: Invalid output options (1 issue found)\n- For the "hoistTransitiveImports". Invalid type: Expected false but received true. \x1b[0m`,
  );
});

test('requires a string for codeSplitting group debugName', async () => {
  const consoleSpy = vi.spyOn(console, 'warn');
  const bundle = await rolldown({
    input: './build-api/main.js',
    cwd: import.meta.dirname,
  });
  try {
    await bundle.generate({
      codeSplitting: {
        groups: [
          {
            // @ts-ignore invalid value
            debugName: 1,
            name: 'group',
          },
        ],
      },
    });
    expect(consoleSpy).toHaveBeenCalledWith(
      expect.stringContaining('For the "codeSplitting.groups,0,debugName"'),
    );
    expect(consoleSpy).toHaveBeenCalledWith(
      expect.stringContaining('Invalid type: Expected string but received 1'),
    );
  } finally {
    await bundle.close();
  }
});

test('requires devtools.sessionId to be one path segment', async () => {
  const consoleSpy = vi.spyOn(console, 'warn');
  for (const sessionId of ['../x', 'a/b', '', '.']) {
    consoleSpy.mockClear();
    const bundle = await rolldown({
      input: './build-api/main.js',
      cwd: import.meta.dirname,
      devtools: { sessionId },
    });
    try {
      expect(consoleSpy).toHaveBeenCalledWith(
        expect.stringContaining('For the "devtools.sessionId". Expected one path segment'),
      );
      // The warning alone would still let the build write outside `node_modules/.rolldown`.
      await expect(bundle.generate()).rejects.toThrow(
        'Invalid value for option "devtools.sessionId"',
      );
    } finally {
      await bundle.close();
    }
  }
});

test('rejects the reserved devtools.sessionId', async () => {
  const consoleSpy = vi.spyOn(console, 'warn');
  for (const sessionId of [
    'unknown-session',
    'UNKNOWN-SESSION',
    'unKnown-session',
    'unknown-ſession',
    'unknown-seßion',
    'unknown-seẞion',
  ]) {
    consoleSpy.mockClear();
    const bundle = await rolldown({
      input: './build-api/main.js',
      cwd: import.meta.dirname,
      devtools: { sessionId },
    });
    try {
      expect(consoleSpy).toHaveBeenCalledWith(
        expect.stringContaining(
          'For the "devtools.sessionId". "unknown-session" is reserved for devtools events without a session context',
        ),
      );
      await expect(bundle.generate()).rejects.toThrow(
        'is reserved for devtools events without a session context',
      );
    } finally {
      await bundle.close();
    }
  }
});
