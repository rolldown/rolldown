import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { rolldown } from 'rolldown';
import { BindingBundler, BindingLogLevel } from '../../../src/binding.cjs';

const cwd = mkdtempSync(path.join(tmpdir(), 'rolldown-devtools-close-'));
const closeError = Object.assign(new RangeError('devtools closeBundle identity'), {
  marker: 'original-close-error',
});
let closeBundleCalls = 0;
let directCloseBundleCalls = 0;

function directBindingOptions(name, closeBundle, devtools) {
  return {
    inputOptions: {
      cwd,
      devtools,
      input: [{ import: './main.js' }],
      logLevel: BindingLogLevel.Silent,
      onLog() {},
      plugins: [{ name, hookUsage: 1 << 13, closeBundle }],
    },
    outputOptions: { plugins: [] },
  };
}

try {
  mkdirSync(path.join(cwd, 'node_modules'), { recursive: true });
  writeFileSync(path.join(cwd, 'node_modules', '.rolldown'), 'blocks devtools output directory');
  writeFileSync(path.join(cwd, 'main.js'), 'export const value = 1;\n');
  process.chdir(cwd);

  const build = await rolldown({
    cwd,
    devtools: {},
    input: './main.js',
    plugins: [
      {
        name: 'devtools-close-error',
        closeBundle() {
          closeBundleCalls += 1;
          throw closeError;
        },
      },
    ],
  });
  await build.generate();

  const first = build.close();
  const concurrent = build.close();
  const concurrentPromiseReused = concurrent === first;
  const firstError = await first.catch((error) => error);
  const concurrentError = await concurrent.catch((error) => error);

  assert(firstError instanceof AggregateError);
  assert.equal(concurrentError, firstError);
  assert(firstError.errors.length > 1);
  assert.equal(firstError.errors[0], closeError);
  const writerErrors = firstError.errors.slice(1);
  assert(writerErrors.every((error) => error.code === 'BUNDLER_CLOSE_ERROR'));
  assert(writerErrors.every((error) => /devtools|rolldown log/i.test(error.message)));

  const late = build.close();
  const lateError = await late.catch((error) => error);
  assert.equal(late, first);
  assert.equal(lateError, firstError);
  assert.equal(closeBundleCalls, 1);

  const directCloseError = Object.assign(new SyntaxError('direct binding closeBundle identity'), {
    marker: 'direct-binding-close-error',
  });
  const directBundler = new BindingBundler();
  const directGenerateResult = await directBundler.generate(
    directBindingOptions(
      'direct-binding-devtools-close-error',
      () => {
        directCloseBundleCalls += 1;
        throw directCloseError;
      },
      { sessionId: 'direct-binding-session' },
    ),
  );
  assert.equal(directGenerateResult?.isBindingErrors, undefined);

  const directError = await directBundler.close().then(
    () => null,
    (error) => error,
  );
  assert(directError instanceof AggregateError);
  assert.equal(directError.errors[0], directCloseError);
  const directWriterErrors = directError.errors.slice(1);
  assert(directWriterErrors.length > 0);
  assert(directWriterErrors.every((error) => error instanceof Error));
  assert(directWriterErrors.every((error) => error.code === 'BUNDLER_CLOSE_ERROR'));
  assert(directWriterErrors.every((error) => /devtools|rolldown log/i.test(error.message)));
  assert.equal(directCloseBundleCalls, 1);

  const loneCloseError = new URIError('direct binding lone closeBundle identity');
  const loneBundler = new BindingBundler();
  const loneGenerateResult = await loneBundler.generate(
    directBindingOptions('direct-binding-lone-close-error', () => {
      throw loneCloseError;
    }),
  );
  assert.equal(loneGenerateResult?.isBindingErrors, undefined);
  const loneRejection = await loneBundler.close().then(
    () => null,
    (error) => error,
  );
  assert.equal(loneRejection, loneCloseError);

  console.log(
    JSON.stringify({
      closeBundleCalls,
      concurrentPromiseReused,
      directBindingFailuresPreserved:
        directError.errors[0] === directCloseError && directWriterErrors.length > 0,
      directCloseBundleCalls,
      loneDirectErrorIdentityPreserved: loneRejection === loneCloseError,
      originalErrorPreserved: firstError.errors[0] === closeError,
      replayedAggregatePreserved: concurrentError === firstError && lateError === firstError,
      writerErrorsPreserved: writerErrors.length > 0,
    }),
  );
} finally {
  process.chdir(tmpdir());
  rmSync(cwd, { force: true, recursive: true });
}
