// @ts-nocheck This focused unit test mocks the generated binding surface.
//
// `parseAst()`/`parseAstAsync()` hand the result of `src/utils/parse.ts` to
// `wrap()` in `src/parse-ast-index.ts`. On the threadless-WASI flavor
// `wrapParseResult` has ALREADY drained all four native getters before `wrap()`
// sees the result, so `wrap()` must not touch the wrapper's `program` getter:
// that getter is a lazy `jsonParseAst` over the serialized AST, and the error
// path throws the deserialized program away immediately.
//
// A failed parse still carries a whole program: `showSemanticErrors` reports
// errors on a fully parsed AST, so the payload below (captured verbatim from
// the native binding for `MALFORMED_SOURCE`) has both a real `program` and a
// real `errors` entry. Measured on a real file the serialized AST reaches
// hundreds of KB of JSON, all of it deserialized and discarded.
//
// Two halves must hold together, or the guarantee is only half kept:
//   a. `jsonParseAst` is never called on the error path (time), and
//   b. every native getter is still read exactly once (memory -- threadless
//      WASI never runs the finalizers that would otherwise free them).
//
// See also `utility-parse-eager-free.test.ts`, which covers the drain itself in
// `src/utils/parse.ts` with the real, unmocked `oxc-parser/src-js/wrap.js`.
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

// The only mocked piece of oxc-parser is a counting passthrough around
// `jsonParseAst`: the real implementation still runs, so the success path keeps
// its BigInt/RegExp revival exactly as in production. `wrap()` is re-exported
// untouched (it calls its own module-local `jsonParseAst`, which this spy
// deliberately does not intercept -- only the eager path in
// `src/utils/parse.ts` reaches `jsonParseAst` through the module namespace).
const oxcParserWrapSpies = vi.hoisted(() => ({ jsonParseAst: vi.fn() }));

vi.mock('oxc-parser/src-js/wrap.js', async (importOriginal) => {
  const actual = await importOriginal();
  oxcParserWrapSpies.jsonParseAst.mockImplementation(actual.jsonParseAst);
  return { ...actual, jsonParseAst: oxcParserWrapSpies.jsonParseAst };
});

// Duplicate `const a` -- invalid JavaScript, reported only with
// `showSemanticErrors`, which is the case where a failed parse still carries a
// complete AST.
const MALFORMED_SOURCE = 'const a = 1n;\nconst a = /xy/g;\n';

// Captured verbatim from `binding.parseSync('malformed.js', MALFORMED_SOURCE,
// { lang: 'js', preserveParens: false, showSemanticErrors: true })`.
const PROGRAM_JSON =
  '{"node":\n{"type":"Program","body":[{"type":"VariableDeclaration","kind":"const","declarations":' +
  '[{"type":"VariableDeclarator","id":{"type":"Identifier","name":"a","start":6,"end":7},"init":' +
  '{"type":"Literal","value":null,"raw":"1n","bigint":"1","start":10,"end":12},"start":6,"end":12}],' +
  '"start":0,"end":13},{"type":"VariableDeclaration","kind":"const","declarations":' +
  '[{"type":"VariableDeclarator","id":{"type":"Identifier","name":"a","start":20,"end":21},"init":' +
  '{"type":"Literal","value":null,"raw":"/xy/g","regex":{"pattern":"xy","flags":"g"},"start":24,' +
  '"end":29},"start":20,"end":29}],"start":14,"end":30}],"sourceType":"script","hashbang":null,' +
  '"start":0,"end":31}\n,"fixes":[["body",0,"declarations",0,"init"],["body",1,"declarations",0,"init"]]}';

const SEMANTIC_ERRORS = [
  {
    severity: 'Error',
    message: 'Identifier `a` has already been declared',
    labels: [
      { message: '`a` has already been declared here', start: 6, end: 7 },
      { message: 'It can not be redeclared here', start: 20, end: 21 },
    ],
    helpMessage: null,
    codeframe: '\n  x Identifier `a` has already been declared\n   ,-[malformed.js:1:7]\n',
  },
];

// The exact error the real `parseAst()` throws for `MALFORMED_SOURCE` today.
const EXPECTED_MESSAGE =
  'Parse failed with 1 error:\n' +
  'Identifier `a` has already been declared\n' +
  '1: const a = 1n;\n' +
  '         ^\n' +
  '2: const a = /xy/g;\n' +
  '1: const a = 1n;\n' +
  '2: const a = /xy/g;\n' +
  '         ^';
const EXPECTED_FRAME = '1: const a = 1n;\n         ^\n2: const a = /xy/g;';

// A fake upstream `ParseResult`: every getter counts its reads and drains on
// the first one, exactly like the napi class's `mem::take` getters.
function createNativeParseResult(errors) {
  const reads = { program: 0, module: 0, comments: 0, errors: 0 };
  const values = {
    program: PROGRAM_JSON,
    module: {
      hasModuleSyntax: false,
      staticImports: [],
      staticExports: [],
      dynamicImports: [],
      importMetas: [],
    },
    comments: [],
    errors,
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

async function loadThreadlessWasiParseAst() {
  binding.__rolldownBindingTarget = 'wasi';
  binding.target = 'wasi';
  vi.resetModules();
  oxcParserWrapSpies.jsonParseAst.mockClear();
  const [parseAstIndex, { shouldEagerlyFreeOutputs }] = await Promise.all([
    import('../src/parse-ast-index'),
    import('../src/utils/threadless-free'),
  ]);
  // Guard the premise so every assertion below is about the eager path.
  expect(shouldEagerlyFreeOutputs()).toBe(true);
  return parseAstIndex;
}

function captureThrow(run) {
  try {
    run();
  } catch (error) {
    return error;
  }
  throw new Error('expected the call to throw a parse error');
}

async function captureRejection(run) {
  try {
    await run();
  } catch (error) {
    return error;
  }
  throw new Error('expected the call to reject with a parse error');
}

function expectParseError(thrown) {
  expect(thrown.name).toBe('RolldownError');
  expect(thrown.code).toBe('PARSE_ERROR');
  expect(thrown.id).toBe('malformed.js');
  expect(thrown.pos).toBe(6);
  expect(thrown.loc).toEqual({ column: 6, file: 'malformed.js', line: 1 });
  expect(thrown.frame).toBe(EXPECTED_FRAME);
  expect(thrown.message).toBe(EXPECTED_MESSAGE);
}

test('threadless WASI parseAst error path drains natively without deserializing the program', async () => {
  const { parseAst, parseAstAsync } = await loadThreadlessWasiParseAst();

  const sync = createNativeParseResult(SEMANTIC_ERRORS);
  binding.nextNative = sync.native;
  expectParseError(
    captureThrow(() => parseAst(MALFORMED_SOURCE, { showSemanticErrors: true }, 'malformed.js')),
  );
  // (b) memory: the native storage is released even though the parse failed.
  expect(sync.reads).toEqual({ program: 1, module: 1, comments: 1, errors: 1 });
  // (a) time: nothing revived the AST the error path throws away.
  expect(oxcParserWrapSpies.jsonParseAst).not.toHaveBeenCalled();

  const async = createNativeParseResult(SEMANTIC_ERRORS);
  binding.nextNative = async.native;
  expectParseError(
    await captureRejection(() =>
      parseAstAsync(MALFORMED_SOURCE, { showSemanticErrors: true }, 'malformed.js'),
    ),
  );
  expect(async.reads).toEqual({ program: 1, module: 1, comments: 1, errors: 1 });
  expect(oxcParserWrapSpies.jsonParseAst).not.toHaveBeenCalled();
});

test('threadless WASI parseAst success path still deserializes the program exactly once', async () => {
  // Positive control: proves the `jsonParseAst` spy above is actually wired
  // into the eager path, so the error path's zero-call assertion cannot pass
  // vacuously. It also pins the success path's own drain and revival.
  const { parseAst } = await loadThreadlessWasiParseAst();

  const success = createNativeParseResult([]);
  binding.nextNative = success.native;
  const program = parseAst(MALFORMED_SOURCE, { showSemanticErrors: true }, 'malformed.js');

  expect(oxcParserWrapSpies.jsonParseAst).toHaveBeenCalledTimes(1);
  expect(success.reads).toEqual({ program: 1, module: 1, comments: 1, errors: 1 });
  expect(program.type).toBe('Program');
  // The BigInt/RegExp fixes of the real `jsonParseAst` still run.
  expect(program.body[0].declarations[0].init.value).toBe(1n);
  expect(program.body[1].declarations[0].init.value).toEqual(/xy/g);
});
