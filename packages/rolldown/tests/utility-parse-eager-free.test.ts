// @ts-nocheck This focused unit test mocks the generated binding surface.
//
// The public `parse()`/`parseSync()` utilities return the upstream napi
// `ParseResult`, whose four getters are all `mem::take` drains with no
// `dropInner`. On the threadless-WASI flavor finalizers never run, so any
// field left unread stays allocated forever -- `src/utils/parse.ts` must
// drain all four eagerly there while staying behaviorally identical to the
// lazy oxc-parser wrap object used everywhere else. The real
// `oxc-parser/src-js/wrap.js` is NOT mocked: both flavors must go through
// the same `jsonParseAst` revival (BigInt/RegExp fixes included).
import { expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => {
  const result = {
    __rolldownBindingTarget: 'native',
    target: 'native',
    // The fake native ParseResult handed back by the next parse/parseSync.
    nextNative: undefined,
    getRuntimeCapabilities: vi.fn(() => ({
      asyncRuntimeBuild: false,
      backend: 'tokio',
      blockOnJsThreadSafe: false,
      devSupported: binding.target === 'native',
      flavor: binding.target === 'wasi' ? 'CurrentThread' : 'MultiThread',
      target: binding.target,
      threads: binding.target !== 'wasi',
      timers: binding.target !== 'wasi',
      wasi: binding.target !== 'native',
      watchSupported: binding.target === 'native',
    })),
    parse: vi.fn(async () => binding.nextNative),
    parseSync: vi.fn(() => binding.nextNative),
  };
  return result;
});

vi.mock('../src/binding.cjs', () => binding);

// Serialized-AST payload in the exact `{ node, fixes }` shape the native
// `program` getter returns, with one BigInt and one RegExp literal so both
// `applyFix` branches of wrap.js must run identically on either flavor.
const PROGRAM_NODE = {
  type: 'Program',
  start: 0,
  end: 12,
  sourceType: 'module',
  body: [
    {
      type: 'ExpressionStatement',
      start: 0,
      end: 5,
      expression: { type: 'Literal', start: 0, end: 4, value: null, raw: '123n', bigint: '123' },
    },
    {
      type: 'ExpressionStatement',
      start: 6,
      end: 12,
      expression: {
        type: 'Literal',
        start: 6,
        end: 11,
        value: null,
        raw: '/xy/g',
        regex: { pattern: 'xy', flags: 'g' },
      },
    },
  ],
};
const PROGRAM_JSON = JSON.stringify({
  node: PROGRAM_NODE,
  fixes: [
    ['body', 0, 'expression'],
    ['body', 1, 'expression'],
  ],
});

// A fake upstream `ParseResult`: every getter counts its reads and drains on
// the first one, exactly like the napi class's `mem::take` getters.
function createNativeParseResult() {
  const reads = { program: 0, module: 0, comments: 0, errors: 0 };
  const values = {
    program: PROGRAM_JSON,
    module: {
      hasModuleSyntax: true,
      staticImports: [],
      staticExports: [],
      dynamicImports: [],
      importMetas: [],
    },
    comments: [{ type: 'Line', value: ' note', start: 0, end: 7 }],
    errors: [
      {
        severity: 'Error',
        message: 'Unexpected token',
        labels: [{ message: 'here', start: 0, end: 1 }],
      },
    ],
  };
  const native = {};
  for (const key of ['program', 'module', 'comments', 'errors']) {
    Object.defineProperty(native, key, {
      enumerable: true,
      get() {
        reads[key] += 1;
        const value = values[key];
        // Simulate the upstream mem::take: a second read gets the default.
        values[key] = key === 'program' ? '' : Array.isArray(value) ? [] : undefined;
        return value;
      },
    });
  }
  return { native, reads };
}

function snapshotFields(result) {
  return {
    program: result.program,
    module: result.module,
    comments: result.comments,
    errors: result.errors,
  };
}

test('lazy flavors keep the oxc-parser wrap semantics (no eager native reads)', async () => {
  binding.__rolldownBindingTarget = 'native';
  binding.target = 'native';
  vi.resetModules();
  const [{ parseSync }, { shouldEagerlyFreeOutputs }] = await Promise.all([
    import('../src/utils/parse'),
    import('../src/utils/threadless-free'),
  ]);
  // Guard the premise so the zero-reads assertion cannot pass vacuously.
  expect(shouldEagerlyFreeOutputs()).toBe(false);

  const { native, reads } = createNativeParseResult();
  binding.nextNative = native;
  const result = parseSync('input.js', '123n;/xy/g;');

  // Nothing drained until the caller reads a field.
  expect(reads).toEqual({ program: 0, module: 0, comments: 0, errors: 0 });

  const program = result.program;
  expect(reads).toEqual({ program: 1, module: 0, comments: 0, errors: 0 });
  expect(program.body[0].expression.value).toBe(123n);
  expect(program.body[1].expression.value).toEqual(/xy/g);
  // wrap.js memoizes: a second read must not touch the drained native getter.
  expect(result.program).toBe(program);
  expect(result.module.hasModuleSyntax).toBe(true);
  expect(result.comments).toHaveLength(1);
  expect(result.errors).toHaveLength(1);
  expect(reads).toEqual({ program: 1, module: 1, comments: 1, errors: 1 });
});

test('threadless WASI drains every native field eagerly, behavior unchanged', async () => {
  // Baseline: the lazy flavor's observable result, read through the real
  // wrap.js revival path.
  binding.__rolldownBindingTarget = 'native';
  binding.target = 'native';
  vi.resetModules();
  const lazyParse = await import('../src/utils/parse');
  const lazyNative = createNativeParseResult();
  binding.nextNative = lazyNative.native;
  const baseline = snapshotFields(lazyParse.parseSync('input.js', '123n;/xy/g;'));

  // Forced flag: threadless-WASI capability report.
  binding.__rolldownBindingTarget = 'wasi';
  binding.target = 'wasi';
  vi.resetModules();
  const [{ parse, parseSync }, { shouldEagerlyFreeOutputs }] = await Promise.all([
    import('../src/utils/parse'),
    import('../src/utils/threadless-free'),
  ]);
  // Guard the premise so the drain assertions cannot pass vacuously.
  expect(shouldEagerlyFreeOutputs()).toBe(true);

  const { native, reads } = createNativeParseResult();
  binding.nextNative = native;
  const eager = parseSync('input.js', '123n;/xy/g;');

  // Every native field was drained before the caller touched the result.
  expect(reads).toEqual({ program: 1, module: 1, comments: 1, errors: 1 });

  // The program JSON.parse stays lazy and memoized on the snapshot...
  const program = eager.program;
  expect(eager.program).toBe(program);
  // ...and no read goes back to the (drained) native object.
  expect(reads).toEqual({ program: 1, module: 1, comments: 1, errors: 1 });

  // Behavioral equivalence with the lazy flavor, BigInt/RegExp fixes included.
  expect(snapshotFields(eager)).toEqual(baseline);
  expect(program.body[0].expression.value).toBe(123n);
  expect(program.body[1].expression.value).toEqual(/xy/g);

  // The async utility drains through the same helper, lease path untouched.
  const asyncNative = createNativeParseResult();
  binding.nextNative = asyncNative.native;
  const eagerAsync = await parse('input.js', '123n;/xy/g;');
  expect(asyncNative.reads).toEqual({ program: 1, module: 1, comments: 1, errors: 1 });
  expect(snapshotFields(eagerAsync)).toEqual(baseline);
});
