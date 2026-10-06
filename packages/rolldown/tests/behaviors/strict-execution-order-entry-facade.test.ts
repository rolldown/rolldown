import type { OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// `strictExecutionOrder` moves an entry's body into `init_E()` and puts a top-level `init_E()` call
// in the entry chunk. That inline trigger is only safe while the chunk hosting `E` is loaded solely
// to enter `E`; when something else can load it, the trigger has to move into a facade — an extra
// chunk that holds no modules at all.
//
// Emitting that facade unconditionally is invisible to output-correctness tests: the bundle still
// runs correctly, it just costs one extra file per entry, which on a real app with lazy routes is a
// ~50% chunk-count increase. These tests pin the *shape* of the output, in both directions: no
// facade when nothing else can load the chunk, and a facade when something can.

function virtualPlugin(modules: Record<string, string>): Plugin {
  return {
    name: 'virtual',
    resolveId(id) {
      return id in modules ? id : undefined;
    },
    load(id) {
      return modules[id];
    },
  };
}

type Mode = 'off' | 'wrap-all' | 'on-demand';

async function build(
  modules: Record<string, string>,
  input: Record<string, string>,
  mode: Mode,
  groups?: { name: string; test: RegExp }[],
  moduleSideEffects?: boolean,
): Promise<OutputChunk[]> {
  const bundle = await rolldown({
    input,
    plugins: [virtualPlugin(modules)],
    ...(mode === 'on-demand' ? { experimental: { onDemandWrapping: true } } : {}),
    ...(moduleSideEffects === undefined ? {} : { treeshake: { moduleSideEffects } }),
  });
  const { output } = await bundle.generate({
    format: 'esm',
    ...(mode === 'off' ? {} : { strictExecutionOrder: true }),
    ...(groups ? { codeSplitting: { groups } } : {}),
  });
  await bundle.close();
  return output.filter((chunk): chunk is OutputChunk => chunk.type === 'chunk');
}

/** A facade holds no modules: it exists only to host an entry's `init_*()` trigger. */
function facadeNames(chunks: OutputChunk[]): string[] {
  return chunks
    .filter((chunk) => chunk.moduleIds.length === 0)
    .map((chunk) => chunk.name)
    .sort();
}

const ROUTES = ['a', 'b', 'c', 'd', 'e', 'f'];

// An app whose lazy routes share a library. Nothing outside a route can load that route's chunk, so
// no entry here needs its trigger moved into a facade.
const lazyRouteApp: Record<string, string> = {
  './lib.js': `console.log('lib');\nexport const version = String(globalThis.__v ?? '1');`,
  './early.js': `console.log('early');\nexport const early = 1;`,
  './main.js': `import './early.js';\nimport { version } from './lib.js';\nconsole.log('main', version);\n${ROUTES.map(
    (r) => `await import('./route-${r}.js');`,
  ).join('\n')}`,
  ...Object.fromEntries(
    ROUTES.map((r) => [
      `./route-${r}.js`,
      `import { version } from './lib.js';\nconsole.log('route-${r}', version);\nexport const name = '${r}';`,
    ]),
  ),
};

// Grouping `early.js` with `lib.js` makes the chunk-driven evaluation order differ from source
// order, which is what puts modules on `onDemandWrapping`'s plan. Without it on-demand finds no
// hazard, wraps nothing, and the shape would not exercise entry splitting at all.
const lazyRouteGroups = [{ name: 'grp', test: /(?:lib|early)\.js$/ }];

test.each<Mode>(['wrap-all', 'on-demand'])(
  'strictExecutionOrder (%s) does not emit an entry facade per lazy route',
  async (mode) => {
    const off = await build(lazyRouteApp, { main: './main.js' }, 'off', lazyRouteGroups);
    const strict = await build(lazyRouteApp, { main: './main.js' }, mode, lazyRouteGroups);

    // Nothing here can load a route chunk except entering that route, so no entry needs its
    // trigger moved out. Every extra chunk would be one empty file per lazy route.
    expect(facadeNames(strict)).toEqual([]);

    // Same statement from the other side, so a facade that grew a module still fails: the only
    // chunk strict mode may add is the shared runtime holding `__esmMin`. A per-entry split shows
    // up here as a second chunk carrying the entry's name (`route-a` plus `route-a2`, ...).
    const addedByStrict = strict
      .map((chunk) => chunk.name)
      .filter((name) => name !== 'rolldown-runtime')
      .sort();
    expect(addedByStrict).toEqual(off.map((chunk) => chunk.name).sort());
  },
);

// The counterpart, so the check above can never be satisfied by simply never splitting: entry `e`
// shares a chunk with `shared.js`, and entry `b` imports `shared.js`. Loading that chunk for `b`
// must not run `e`'s program, so `e`'s trigger has to move into a facade.
const sharedEntryChunkApp: Record<string, string> = {
  './shared.js': `console.log('shared');\nexport const s = String(globalThis.__s ?? 's');`,
  './b.js': `import { s } from './shared.js';\nconsole.log('b', s);`,
  './e.js': `console.log('e');`,
};

test.each<Mode>(['wrap-all', 'on-demand'])(
  'strictExecutionOrder (%s) still splits the entry facade when another chunk imports the entry chunk',
  async (mode) => {
    const chunks = await build(sharedEntryChunkApp, { b: './b.js', e: './e.js' }, mode, [
      { name: 'e-group', test: /(?:e|shared)\.js$/ },
    ]);

    expect(facadeNames(chunks)).toEqual(['e']);

    // The facade took over the entry role; the chunk that actually holds `e.js` is no longer an
    // entry, so nothing calls `init_e()` when `b` pulls that chunk in for `shared.js`.
    const implementation = chunks.find((chunk) =>
      chunk.moduleIds.some((id) => id.endsWith('e.js')),
    );
    expect(implementation?.isEntry).toBe(false);
    expect(implementation?.moduleIds.map((id) => id.replace(/^.*[\\/]/, '')).sort()).toEqual([
      'e.js',
      'shared.js',
    ]);
  },
);

// A dynamic import whose entry tree shaking dropped is rewritten to an inert
// `Promise.resolve().then(...)` stub, so it loads nothing and must not count as a load. Here `b`'s
// only reference to `t.js` is such a record, while `e` uses `t.js` statically and so shares a chunk
// with it — the shape where a dead record would otherwise keep a facade alive.
const deadDynamicApp: Record<string, string> = {
  // `String(...)` keeps the export from being constant-inlined, which would drop `t.js` entirely.
  './t.js': `export const t = String(globalThis.__t ?? 't');`,
  './e.js': `import { t } from './t.js';\nconsole.log('E', t);`,
  './b.js': `import('./t.js');\nconsole.log('B');`,
};

test.each<Mode>(['wrap-all', 'on-demand'])(
  'strictExecutionOrder (%s) does not let a dead dynamic import force an entry facade',
  async (mode) => {
    const chunks = await build(
      deadDynamicApp,
      { b: './b.js', e: './e.js' },
      mode,
      undefined,
      false,
    );

    expect(facadeNames(chunks)).toEqual([]);
  },
);
