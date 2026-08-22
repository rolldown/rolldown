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
import v8 from 'node:v8';
import vm from 'node:vm';
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
function createNativeParseResult(programValue = PROGRAM_JSON) {
  const reads = { program: 0, module: 0, comments: 0, errors: 0 };
  const values = {
    program: programValue,
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

// -- Releasing the serialized AST after revival ------------------------------
//
// Once `jsonParseAst` has produced the object graph, the JSON string it came
// from is dead weight: measured on real files it runs 2-19x the source size
// (877 KB for this package's own 69 KB `dist/utils-index.mjs`, 76 MB for
// TypeScript's 8.7 MB `typescript.js`), so a caller that keeps a `parseSync()`
// result and reads `.program` would hold the whole serialized AST *and* the
// whole deserialized one -- exactly the doubling the eager drain exists to
// avoid. `parseAst()` is unaffected: `wrap()` in `src/parse-ast-index.ts`
// returns `result.program` and lets the wrapper (and its closure) die.
//
// Reachability is the only observable difference between holding that string
// and dropping it, so the tests below instrument the JSON as an object rather
// than a primitive: `jsonParseAst` does `JSON.parse(programJson)`, which
// coerces its argument, so a `String` box behaves exactly like the primitive
// the native getter really hands out -- but unlike a primitive it can be the
// target of a `WeakRef`, and a plain object with a stateful `toString()` can
// make the revival fail on demand.

// The vitest worker is not started with `--expose-gc`, so borrow a `gc()` the
// way node's own memory tests do.
const gc =
  globalThis.gc ??
  (() => {
    v8.setFlagsFromString('--expose-gc');
    try {
      return vm.runInNewContext('gc');
    } finally {
      v8.setFlagsFromString('--no-expose-gc');
    }
  })();

// A `WeakRef` target is kept alive for the remainder of the current job, so a
// macrotask boundary has to pass before `deref()` can ever report a collection.
async function collect() {
  gc();
  await new Promise((resolve) => setTimeout(resolve, 0));
  gc();
}

async function loadThreadlessWasiParse() {
  binding.__rolldownBindingTarget = 'wasi';
  binding.target = 'wasi';
  vi.resetModules();
  const [parse, { shouldEagerlyFreeOutputs }] = await Promise.all([
    import('../src/utils/parse'),
    import('../src/utils/threadless-free'),
  ]);
  // Guard the premise so every assertion below is about the eager path.
  expect(shouldEagerlyFreeOutputs()).toBe(true);
  return parse;
}

function captureThrow(run) {
  try {
    run();
  } catch (error) {
    return error;
  }
  throw new Error('expected the getter to throw');
}

test('threadless WASI releases the serialized AST once it has been revived', async () => {
  const { parseSync } = await loadThreadlessWasiParse();

  // The boxed JSON is created inside a scope that keeps no strong reference of
  // its own: after this IIFE returns, the only paths to it are the wrapper's
  // closure and the `WeakRef`. The fake native result drains to `''` on the
  // eager read, so it is not one of them.
  let eager, reads;
  const jsonRef = (() => {
    const box = new String(PROGRAM_JSON);
    const native = createNativeParseResult(box);
    reads = native.reads;
    binding.nextNative = native.native;
    eager = parseSync('input.js', '123n;/xy/g;');
    return new WeakRef(box);
  })();

  expect(reads).toEqual({ program: 1, module: 1, comments: 1, errors: 1 });

  // Positive control: before anything reads `.program` the wrapper genuinely
  // is holding the serialized AST, so the WeakRef must NOT have cleared. Its
  // clearing below therefore means the revival released it, not that the box
  // was unreachable all along.
  await collect();
  expect(jsonRef.deref()).toBeDefined();

  const program = eager.program;
  expect(program.body[0].expression.value).toBe(123n);
  expect(program.body[1].expression.value).toEqual(/xy/g);
  // Memoized: repeated reads hand back the identical object graph.
  expect(eager.program).toBe(program);

  // The caller still holds the wrapper, yet the JSON is gone.
  await collect();
  expect(jsonRef.deref()).toBeUndefined();
  // ...and the memo survived the release.
  expect(eager.program).toBe(program);
});

test('threadless WASI keeps the serialized AST when the revival throws', async () => {
  const { parseSync } = await loadThreadlessWasiParse();

  // `JSON.parse(programJson)` coerces its argument, so this box fails the
  // first revival and succeeds on every later one.
  const failure = new Error('revival exploded');
  let coercions = 0;
  const box = {
    toString() {
      coercions += 1;
      if (coercions === 1) throw failure;
      return PROGRAM_JSON;
    },
  };
  const { native, reads } = createNativeParseResult(box);
  binding.nextNative = native;
  const eager = parseSync('input.js', '123n;/xy/g;');
  expect(reads).toEqual({ program: 1, module: 1, comments: 1, errors: 1 });

  // The failure propagates untouched -- same error object, not a wrapper.
  expect(captureThrow(() => eager.program)).toBe(failure);
  expect(coercions).toBe(1);

  // Nothing was released, so the next read retries and succeeds.
  const program = eager.program;
  expect(coercions).toBe(2);
  expect(program.body[0].expression.value).toBe(123n);
  expect(program.body[1].expression.value).toEqual(/xy/g);

  // The successful retry memoizes in turn: no third coercion.
  expect(eager.program).toBe(program);
  expect(coercions).toBe(2);
});

test('threadless WASI memoizes a falsy revival without re-reading the released JSON', async () => {
  // `jsonParseAst` returns `JSON.parse(programJson).node`, so a payload whose
  // `node` is null revives to a falsy value. Gating the memo on the AST being
  // truthy (`if (!program)`) would send the second read back to a JSON string
  // that release has already cleared -- `JSON.parse(undefined)` throws.
  const { parseSync } = await loadThreadlessWasiParse();

  const { native, reads } = createNativeParseResult(JSON.stringify({ node: null, fixes: [] }));
  binding.nextNative = native;
  const eager = parseSync('input.js', '');
  expect(reads).toEqual({ program: 1, module: 1, comments: 1, errors: 1 });

  expect(eager.program).toBe(null);
  // Second read must return the same falsy memo, not throw.
  expect(eager.program).toBe(null);
  expect(eager.program).toBe(null);
});
