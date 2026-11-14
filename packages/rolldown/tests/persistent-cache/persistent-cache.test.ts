import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  type InputOptions,
  type OutputOptions,
  type Plugin,
  type PluginContext,
  type RolldownLog,
  rolldown,
  watch,
} from 'rolldown';
import { defineParallelPlugin } from 'rolldown/experimental';
import { isWasiTest, sleep } from 'rolldown-tests/utils';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

type CacheOption = NonNullable<InputOptions['experimental']>['cache'];

let root: string;
let src: string;
let cacheDir: string;

beforeEach(() => {
  // `realpathSync`: on macOS the temp directory is behind a symbolic link, and module ids are
  // real paths.
  root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-persistent-cache-')));
  src = path.join(root, 'src');
  cacheDir = path.join(root, 'cache');
  fs.mkdirSync(src);
});

afterEach(() => {
  fs.rmSync(root, { recursive: true, force: true });
});

function writeFiles(files: Record<string, string>) {
  for (const [name, content] of Object.entries(files)) {
    fs.writeFileSync(path.join(src, name), content);
  }
}

const rel = (id: string) => path.relative(src, id);

const BASIC_FILES = {
  'main.js': `import { a } from './a.js';\nimport { b } from './b.js';\nconsole.log(a, b, __NAME__);\n`,
  'a.js': `import { c } from './c.js';\nexport const a = 'a' + c + __NAME__;\n`,
  'b.js': `export const b = 'b' + __NAME__;\n`,
  'c.js': `export const c = 'c' + __NAME__;\n`,
};

/**
 * A plugin that records its calls for the files in `src` as `hook:module`, and replaces
 * `__NAME__` in `transform`. Rolldown's runtime module is not a file, so it never gets a saved
 * result.
 */
function recordingPlugin(calls: string[], cache: boolean | undefined, name = 'recording'): Plugin {
  return {
    name,
    resolveId: {
      cache,
      handler(source, importer) {
        if (importer?.startsWith(src)) calls.push(`${name}.resolveId:${rel(importer)}:${source}`);
        return null;
      },
    },
    load: {
      cache,
      handler(id) {
        if (id.startsWith(src)) calls.push(`${name}.load:${rel(id)}`);
        return null;
      },
    },
    transform: {
      cache,
      handler(code, id) {
        if (!id.startsWith(src)) return null;
        calls.push(`${name}.transform:${rel(id)}`);
        return code.replaceAll('__NAME__', JSON.stringify(rel(id)));
      },
    },
  };
}

/** What a plugin reads for the files in `src`, by `hook:module`. */
type Seen = Record<string, { meta: unknown; moduleSideEffects: unknown } | null>;

/**
 * A plugin that reads `meta` and `moduleSideEffects` of the files in `src` in hooks that the
 * persistent cache never skips. It copies the values, so a later change does not show up.
 */
function readerPlugin(seen: Seen): Plugin {
  const copy = (info: { meta: unknown; moduleSideEffects: unknown } | null) =>
    info && structuredClone({ meta: info.meta, moduleSideEffects: info.moduleSideEffects });
  const read = (ctx: PluginContext, hook: string, ids: Iterable<string>) => {
    for (const id of ids) {
      if (id.startsWith(src)) seen[`${hook}:${rel(id)}`] = copy(ctx.getModuleInfo(id));
    }
  };
  return {
    name: 'reader',
    moduleParsed(info) {
      if (info.id.startsWith(src)) seen[`moduleParsed:${rel(info.id)}`] = copy(info);
    },
    buildEnd() {
      read(this, 'buildEnd', this.getModuleIds());
    },
    renderChunk(_code, chunk) {
      read(this, 'renderChunk', chunk.moduleIds);
    },
    generateBundle() {
      read(this, 'generateBundle', this.getModuleIds());
    },
  };
}

interface BuildOptions {
  plugins: Plugin[];
  /** Defaults to `{ dir: cacheDir }`. Pass `undefined` explicitly to leave it unset. */
  cache?: CacheOption;
  onLog?: InputOptions['onLog'];
  output?: OutputOptions;
  treeshake?: InputOptions['treeshake'];
}

async function build(options: BuildOptions) {
  const cache = 'cache' in options ? options.cache : { dir: cacheDir };
  const bundle = await rolldown({
    input: path.join(src, 'main.js'),
    cwd: root,
    plugins: options.plugins,
    onLog: options.onLog,
    treeshake: options.treeshake,
    experimental: { cache },
  });
  try {
    const { output } = await bundle.generate({ format: 'esm', ...options.output });
    return output
      .map((file) =>
        file.type === 'chunk'
          ? { type: file.type, fileName: file.fileName, code: file.code }
          : {
              type: file.type,
              fileName: file.fileName,
              names: file.names,
              source: Buffer.from(file.source).toString(),
            },
      )
      .sort((x, y) => x.fileName.localeCompare(y.fileName));
  } finally {
    await bundle.close();
  }
}

/** The output of a build without the cache, with plugins made by `makePlugins`. */
async function cleanBuild(makePlugins: () => Plugin[]) {
  return build({ plugins: makePlugins(), cache: undefined });
}

function listFiles(dir: string): string[] {
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir, { recursive: true, withFileTypes: true })
    .flatMap((entry) => (entry.isFile() ? [path.join(entry.parentPath, entry.name)] : []));
}

describe('persistent cache', () => {
  test('a second build skips cached hooks and gives the same output', async () => {
    writeFiles(BASIC_FILES);
    const first: string[] = [];
    const out1 = await build({ plugins: [recordingPlugin(first, true)] });
    expect(first.filter((c) => c.includes('.transform:')).sort()).toEqual([
      'recording.transform:a.js',
      'recording.transform:b.js',
      'recording.transform:c.js',
      'recording.transform:main.js',
    ]);
    expect(first).toContain('recording.resolveId:main.js:./a.js');

    const second: string[] = [];
    const out2 = await build({ plugins: [recordingPlugin(second, true)] });
    expect(second).toEqual([]);
    expect(out2).toEqual(out1);
    expect(out1).toEqual(await cleanBuild(() => [recordingPlugin([], true)]));
  });

  test('a hook without `cache` runs again only for the modules its filter matches', async () => {
    writeFiles(BASIC_FILES);
    const makePlugins = (calls: string[]): Plugin[] => [
      recordingPlugin(calls, true),
      {
        name: 'not-cached-a',
        transform: {
          filter: { id: /[\\/]a\.js$/ },
          handler(code, id) {
            calls.push(`not-cached-a.transform:${rel(id)}`);
            return code + '\n// not-cached-a\n';
          },
        },
      },
      {
        name: 'not-cached-no-match',
        transform: {
          filter: { id: /does-not-exist/ },
          handler(_code, id) {
            calls.push(`not-cached-no-match.transform:${rel(id)}`);
            return null;
          },
        },
      },
    ];
    const first: string[] = [];
    const out1 = await build({ plugins: makePlugins(first) });
    expect(first).not.toContain('not-cached-no-match.transform:a.js');

    const second: string[] = [];
    const out2 = await build({ plugins: makePlugins(second) });
    expect(second.sort()).toEqual([
      'not-cached-a.transform:a.js',
      'recording.load:a.js',
      'recording.resolveId:a.js:./c.js',
      'recording.transform:a.js',
    ]);
    expect(out2).toEqual(out1);
  });

  test('`assumeCacheableHooks` caches hooks without `cache`, but not hooks with `cache: false`', async () => {
    writeFiles(BASIC_FILES);
    const cache: CacheOption = {
      dir: cacheDir,
      assumeCacheableHooks: { resolveId: true, load: true, transform: true },
    };
    await build({ plugins: [recordingPlugin([], undefined)], cache });
    const unset: string[] = [];
    await build({ plugins: [recordingPlugin(unset, undefined)], cache });
    expect(unset).toEqual([]);

    // A new directory: the saved results above came from other plugin code, which the host
    // must cover with `cacheKey` or `buildDependencies`.
    const offCache: CacheOption = { ...cache, dir: path.join(root, 'cache-off') };
    await build({ plugins: [recordingPlugin([], false)], cache: offCache });
    const off: string[] = [];
    await build({ plugins: [recordingPlugin(off, false)], cache: offCache });
    expect(off.filter((c) => c.includes('.transform:')).sort()).toEqual([
      'recording.transform:a.js',
      'recording.transform:b.js',
      'recording.transform:c.js',
      'recording.transform:main.js',
    ]);
  });

  test('hooks without `cache` run on every build', async () => {
    writeFiles(BASIC_FILES);
    await build({ plugins: [recordingPlugin([], undefined)] });
    const second: string[] = [];
    await build({ plugins: [recordingPlugin(second, undefined)] });
    expect(second.filter((c) => c.includes('.transform:')).length).toBe(4);
  });

  test('editing a file runs the hooks of that module only', async () => {
    writeFiles(BASIC_FILES);
    await build({ plugins: [recordingPlugin([], true)] });
    writeFiles({ 'b.js': `export const b = 'b changed, and longer' + __NAME__;\n` });

    const second: string[] = [];
    const out2 = await build({ plugins: [recordingPlugin(second, true)] });
    expect(second.sort()).toEqual(['recording.load:b.js', 'recording.transform:b.js']);
    expect(out2).toEqual(await cleanBuild(() => [recordingPlugin([], true)]));
  });

  describe('`this.addWatchFile` in `transform`', () => {
    const makePlugins = (calls: string[], dep: string): Plugin[] => [
      recordingPlugin(calls, true),
      {
        name: 'watch-dep',
        transform: {
          cache: true,
          filter: { id: /[\\/]a\.js$/ },
          handler(code, id) {
            calls.push(`watch-dep.transform:${rel(id)}`);
            this.addWatchFile(dep);
            const value = fs.existsSync(dep) ? fs.readFileSync(dep, 'utf8') : 'missing';
            return code + `\nexport const dep = ${JSON.stringify(value)};\n`;
          },
        },
      },
    ];

    async function buildWithDep(dep: string) {
      const calls: string[] = [];
      const output = await build({ plugins: makePlugins(calls, dep) });
      return { calls: calls.filter((c) => c.startsWith('watch-dep.')), output };
    }

    test('editing and deleting the file runs the module again', async () => {
      writeFiles(BASIC_FILES);
      const dep = path.join(root, 'dep.txt');
      fs.writeFileSync(dep, 'one');
      await buildWithDep(dep);
      expect((await buildWithDep(dep)).calls).toEqual([]);

      fs.writeFileSync(dep, 'two, longer');
      const edited = await buildWithDep(dep);
      expect(edited.calls).toEqual(['watch-dep.transform:a.js']);
      expect(edited.output).toEqual(await cleanBuild(() => makePlugins([], dep)));
      expect((await buildWithDep(dep)).calls).toEqual([]);

      fs.rmSync(dep);
      const deleted = await buildWithDep(dep);
      expect(deleted.calls).toEqual(['watch-dep.transform:a.js']);
      expect(deleted.output).toEqual(await cleanBuild(() => makePlugins([], dep)));
    });

    test('creating a file that was missing runs the module again', async () => {
      writeFiles(BASIC_FILES);
      const dep = path.join(root, 'later.txt');
      await buildWithDep(dep);
      expect((await buildWithDep(dep)).calls).toEqual([]);

      fs.writeFileSync(dep, 'now here');
      const created = await buildWithDep(dep);
      expect(created.calls).toEqual(['watch-dep.transform:a.js']);
      expect(created.output).toEqual(await cleanBuild(() => makePlugins([], dep)));
    });
  });

  test('modules whose hooks call `this.load` run on every build', async () => {
    writeFiles({
      'main.js': [
        `import './calls-load.js';`,
        `import './calls-load-parsed.js';`,
        `import './plain.js';`,
        `import './loaded.js';`,
        `console.log('main');`,
        ``,
      ].join('\n'),
      'calls-load.js': `console.log('calls-load');\n`,
      'calls-load-parsed.js': `console.log('calls-load-parsed');\n`,
      'plain.js': `console.log('plain');\n`,
      'loaded.js': `console.log('loaded');\n`,
    });
    const makePlugins = (calls: string[]): Plugin[] => [
      {
        name: 'not-cacheable',
        load: {
          cache: true,
          handler(id) {
            if (!id.startsWith(src)) return null;
            calls.push(`load:${rel(id)}`);
            return null;
          },
        },
        transform: {
          cache: true,
          async handler(_code, id) {
            if (!id.startsWith(src)) return null;
            calls.push(`transform:${rel(id)}`);
            switch (rel(id)) {
              case 'calls-load.js':
                await this.load({ id: path.join(src, 'loaded.js') });
                return null;
              case 'calls-load-parsed.js': {
                // `main.js` is already parsed, so `this.load` returns without calling Rust.
                const info = await this.load({ id: path.join(src, 'main.js') });
                if (info.code === null) throw new Error('main.js is not parsed');
                return null;
              }
              default:
                return null;
            }
          },
        },
      },
    ];
    const expected = [
      'load:calls-load-parsed.js',
      'load:calls-load.js',
      'transform:calls-load-parsed.js',
      'transform:calls-load.js',
    ];
    const out1 = await build({ plugins: makePlugins([]) });
    const second: string[] = [];
    const out2 = await build({ plugins: makePlugins(second) });
    expect(second.sort()).toEqual(expected);
    const third: string[] = [];
    await build({ plugins: makePlugins(third) });
    expect(third.sort()).toEqual(expected);
    expect(out2).toEqual(out1);
  });

  describe('module meta', () => {
    /** Builds without the cache, then two times with it. The last build is the restart. */
    async function buildThreeTimes<S extends object = Seen>(
      makePlugins: (calls: string[], seen: S) => Plugin[],
      options: Pick<BuildOptions, 'treeshake'> = {},
    ) {
      const run = async (cache: CacheOption) => {
        const calls: string[] = [];
        const seen = {} as S;
        const logs: RolldownLog[] = [];
        const output = await build({
          ...options,
          plugins: makePlugins(calls, seen),
          cache,
          onLog: (_level, log) => void logs.push(log),
        });
        return { calls, seen, logs, output };
      };
      const clean = await run(undefined);
      const first = await run({ dir: cacheDir });
      const restart = await run({ dir: cacheDir });
      return { clean, first, restart };
    }

    const moduleInfoWarnings = (logs: RolldownLog[]) =>
      logs.filter((log) => log.code === 'PERSISTENT_CACHE_MODULE_INFO');

    test('`meta` that a cached `transform` returns is read again after a restart', async () => {
      writeFiles(BASIC_FILES);
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        recordingPlugin(calls, true),
        {
          name: 'icons',
          transform: {
            cache: true,
            handler(code, id) {
              if (!id.startsWith(src)) return null;
              calls.push(`icons.transform:${rel(id)}`);
              return {
                code,
                meta: {
                  icons: [rel(id), { nested: [1, 'two', null, true, { deep: {} }] }],
                  shared: 'icons',
                  // JSON leaves this property out.
                  absent: { value: undefined, kept: 0 },
                },
              };
            },
          },
        },
        {
          // Its `shared` replaces the one of `icons`. The other keys stay.
          name: 'second',
          transform: {
            cache: true,
            filter: { id: /[\\/]a\.js$/ },
            handler() {
              return { meta: { shared: 'second', second: 2 } };
            },
          },
        },
        readerPlugin(seen),
      ]);

      expect(clean.seen['renderChunk:a.js']).toEqual({
        meta: {
          icons: ['a.js', { nested: [1, 'two', null, true, { deep: {} }] }],
          shared: 'second',
          second: 2,
          absent: { kept: 0 },
        },
        moduleSideEffects: null,
      });
      expect(clean.seen['renderChunk:b.js']?.meta).toMatchObject({ shared: 'icons' });
      for (const hook of ['moduleParsed', 'buildEnd', 'renderChunk', 'generateBundle']) {
        expect(Object.keys(clean.seen)).toContain(`${hook}:c.js`);
      }

      expect(restart.calls).toEqual([]);
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
      expect(first.output).toEqual(clean.output);
      expect(restart.output).toEqual(clean.output);
      expect(restart.logs).toEqual(clean.logs);
    });

    test('`meta` that a cached `resolveId` returns is on the resolved module after a restart', async () => {
      writeFiles(BASIC_FILES);
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        recordingPlugin(calls, true),
        {
          name: 'resolve-meta',
          resolveId: {
            cache: true,
            handler(source, importer) {
              if (importer && source === './c.js') {
                calls.push(`resolve-meta.resolveId:${rel(importer)}:${source}`);
                return {
                  id: path.join(src, 'c.js'),
                  meta: { 'resolve-meta': { from: rel(importer) } },
                };
              }
              return null;
            },
          },
        },
        readerPlugin(seen),
      ]);

      // `a.js` imports `./c.js`. The meta is for `c.js`, and the saved result of `a.js` holds it.
      expect(clean.seen['renderChunk:c.js']).toEqual({
        meta: { 'resolve-meta': { from: 'a.js' } },
        moduleSideEffects: null,
      });
      expect(clean.seen['moduleParsed:c.js']).toEqual(clean.seen['renderChunk:c.js']);
      expect(clean.seen['renderChunk:a.js']).toEqual({ meta: {}, moduleSideEffects: null });

      expect(restart.calls).toEqual([]);
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
      expect(restart.output).toEqual(clean.output);
    });

    test('`this.resolve` gives the saved `meta` of a module that no hook has read yet', async () => {
      writeFiles(BASIC_FILES);
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        {
          name: 'meta-and-side-effects',
          transform: {
            cache: true,
            filter: { id: /[\\/]c\.js$/ },
            handler(_code, id) {
              calls.push(`transform:${rel(id)}`);
              return { meta: { c: ['saved'] }, moduleSideEffects: 'no-treeshake' };
            },
          },
        },
        {
          name: 'resolver',
          async buildEnd() {
            const resolved = await this.resolve('./c.js', path.join(src, 'a.js'));
            seen['this.resolve:c.js'] = resolved && {
              meta: structuredClone(resolved.meta),
              moduleSideEffects: resolved.moduleSideEffects,
            };
          },
        },
      ]);

      expect(clean.seen).toEqual({
        'this.resolve:c.js': { meta: { c: ['saved'] }, moduleSideEffects: 'no-treeshake' },
      });
      expect(restart.calls).toEqual([]);
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
      expect(restart.output).toEqual(clean.output);
    });

    test('a hook that runs replaces the saved `meta` of an importer that is reused', async () => {
      writeFiles(BASIC_FILES);
      const makePlugins = (calls: string[], seen: Seen): Plugin[] => [
        {
          name: 'same-key',
          resolveId: {
            cache: true,
            handler(source, importer) {
              if (!importer?.startsWith(src)) return null;
              calls.push(`resolveId:${rel(importer)}:${source}`);
              return source === './c.js'
                ? { id: path.join(src, 'c.js'), meta: { key: 'resolveId', other: 'resolveId' } }
                : null;
            },
          },
          transform: {
            cache: true,
            handler(code, id) {
              if (!id.startsWith(src)) return null;
              calls.push(`transform:${rel(id)}`);
              return rel(id) === 'c.js' ? { meta: { key: `transform:${code.length}` } } : null;
            },
          },
        },
        {
          // The first hook that reads `c.js`, after all module tasks.
          name: 'late-reader',
          renderChunk(_code, chunk) {
            for (const id of chunk.moduleIds) {
              if (!id.startsWith(src)) continue;
              const info = this.getModuleInfo(id)!;
              seen[rel(id)] = structuredClone({
                meta: info.meta,
                moduleSideEffects: info.moduleSideEffects,
              });
            }
          },
        },
      ];
      await build({ plugins: makePlugins([], {}) });

      // `a.js` imports `./c.js` and keeps its saved result. `c.js` runs its hooks again.
      const changed = `export const c = 'c, changed and longer' + __NAME__;\n`;
      writeFiles({ 'c.js': changed });
      const calls: string[] = [];
      const seen: Seen = {};
      const output = await build({ plugins: makePlugins(calls, seen) });
      expect(calls).toEqual(['transform:c.js']);

      const cleanSeen: Seen = {};
      const cleanOutput = await build({ plugins: makePlugins([], cleanSeen), cache: undefined });
      expect(cleanSeen['c.js']?.meta).toEqual({
        key: `transform:${changed.length}`,
        other: 'resolveId',
      });
      expect(seen).toEqual(cleanSeen);
      expect(output).toEqual(cleanOutput);
    });

    test('`manualChunks` reads the saved `meta` after a restart', async () => {
      writeFiles(BASIC_FILES);
      const output: OutputOptions = {
        manualChunks: (id, { getModuleInfo }) => getModuleInfo(id)?.meta.group,
      };
      const makePlugins = (calls: string[]): Plugin[] => [
        {
          name: 'groups',
          transform: {
            cache: true,
            handler(_code, id) {
              if (!id.startsWith(src)) return null;
              calls.push(rel(id));
              return rel(id) === 'main.js' ? null : { meta: { group: `group-${rel(id)[0]}` } };
            },
          },
        },
      ];
      const clean = await build({ plugins: makePlugins([]), cache: undefined, output });
      // `c.js` goes into the chunk of `a.js`, its only importer.
      expect(clean.map((file) => file.fileName).sort()).toEqual([
        expect.stringMatching(/^group-a.*\.js$/),
        expect.stringMatching(/^group-b.*\.js$/),
        'main.js',
      ]);

      expect(await build({ plugins: makePlugins([]), output })).toEqual(clean);
      const calls: string[] = [];
      expect(await build({ plugins: makePlugins(calls), output })).toEqual(clean);
      expect(calls).toEqual([]);
    });

    test('`moduleSideEffects` that a cached hook returns is read again after a restart', async () => {
      writeFiles({
        'main.js': [
          `import './load-false.js';`,
          `import './transform-false.js';`,
          `import './transform-no-treeshake.js';`,
          `import './resolve-false.js';`,
          `import './plain.js';`,
          `console.log('main');`,
          ``,
        ].join('\n'),
        'load-false.js': `console.log('load-false');\n`,
        'transform-false.js': `console.log('transform-false');\n`,
        'transform-no-treeshake.js': `export const unused = 1;\n`,
        'resolve-false.js': `console.log('resolve-false');\n`,
        'plain.js': `console.log('plain');\n`,
      });
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        {
          name: 'side-effects',
          resolveId: {
            cache: true,
            handler(source, importer) {
              if (!importer?.startsWith(src)) return null;
              calls.push(`resolveId:${rel(importer)}:${source}`);
              return source === './resolve-false.js'
                ? { id: path.join(src, 'resolve-false.js'), moduleSideEffects: false }
                : null;
            },
          },
          load: {
            cache: true,
            handler(id) {
              if (!id.startsWith(src)) return null;
              calls.push(`load:${rel(id)}`);
              return rel(id) === 'load-false.js'
                ? { code: fs.readFileSync(id, 'utf8'), moduleSideEffects: false }
                : null;
            },
          },
          transform: {
            cache: true,
            handler(code, id) {
              if (!id.startsWith(src)) return null;
              calls.push(`transform:${rel(id)}`);
              switch (rel(id)) {
                case 'transform-false.js':
                  return { code, moduleSideEffects: false };
                case 'transform-no-treeshake.js':
                  return { code, moduleSideEffects: 'no-treeshake' };
                default:
                  return null;
              }
            },
          },
        },
        {
          // A hook that returns an object, but no `moduleSideEffects`, after the hooks above. Its
          // wrapper passes on the value that the module already has.
          name: 'object-result',
          transform: {
            cache: true,
            handler(code, id) {
              return id.startsWith(src) ? { code: `${code}// object-result\n` } : null;
            },
          },
        },
        readerPlugin(seen),
      ]);

      for (const hook of ['moduleParsed', 'buildEnd', 'renderChunk', 'generateBundle']) {
        expect(
          Object.fromEntries(
            Object.entries(clean.seen)
              .filter(([key]) => key.startsWith(`${hook}:`))
              // Rolldown removes a module without side effects before `renderChunk`.
              .map(([key, value]) => [key.slice(hook.length + 1), value?.moduleSideEffects]),
          ),
        ).toEqual({
          'main.js': null,
          'plain.js': null,
          'transform-no-treeshake.js': 'no-treeshake',
          ...(hook === 'renderChunk'
            ? {}
            : { 'load-false.js': false, 'transform-false.js': false, 'resolve-false.js': false }),
        });
      }
      const code = clean.output.map((file) => (file.type === 'chunk' ? file.code : '')).join('\n');
      expect(code).toContain('"plain"');
      expect(code).not.toContain('"load-false"');
      expect(code).not.toContain('"transform-false"');
      expect(code).not.toContain('"resolve-false"');

      // The value of `resolve-false.js` comes from the task of `main.js`, so `resolve-false.js`
      // gets no saved result. The other values come from hooks of the module itself. Passing on
      // the value gives no warning.
      expect(restart.calls.sort()).toEqual(['load:resolve-false.js', 'transform:resolve-false.js']);
      for (const run of [first, restart]) {
        expect(moduleInfoWarnings(run.logs)).toEqual([]);
      }
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
      expect(first.output).toEqual(clean.output);
      expect(restart.output).toEqual(clean.output);
    });

    test('a `meta` value that cannot be saved gives no saved result, and one warning', async () => {
      writeFiles(BASIC_FILES);
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        recordingPlugin(calls, true),
        {
          name: 'icons',
          transform: {
            cache: true,
            handler(_code, id) {
              switch (id.startsWith(src) ? rel(id) : '') {
                // Two modules with the problem, and still one warning.
                case 'a.js':
                case 'c.js':
                  return { meta: { icons: new Set(['home', 'search']) } };
                case 'b.js':
                  return { meta: { icons: ['home', 'search'] } };
                default:
                  return null;
              }
            },
          },
        },
        readerPlugin(seen),
      ]);
      const warnings = (logs: RolldownLog[]) =>
        logs.filter((log) => log.code === 'PERSISTENT_CACHE_UNSAVABLE_META');

      expect(clean.seen['renderChunk:a.js']?.meta).toEqual({ icons: new Set(['home', 'search']) });
      expect(clean.seen['renderChunk:b.js']?.meta).toEqual({ icons: ['home', 'search'] });
      expect(warnings(clean.logs)).toEqual([]);

      // `a.js` and `c.js` have no saved result. `b.js` and `main.js` have one.
      expect(restart.calls.sort()).toEqual([
        'recording.load:a.js',
        'recording.load:c.js',
        'recording.resolveId:a.js:./c.js',
        'recording.transform:a.js',
        'recording.transform:c.js',
      ]);
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
      expect(restart.output).toEqual(clean.output);

      for (const logs of [first.logs, restart.logs]) {
        expect(warnings(logs)).toHaveLength(1);
        expect(warnings(logs)[0]).toMatchObject({ plugin: 'icons', hook: 'transform' });
        expect(warnings(logs)[0].message).toContain('"icons"');
        expect(warnings(logs)[0].message).toContain('"transform"');
        expect([path.join(src, 'a.js'), path.join(src, 'c.js')]).toContain(warnings(logs)[0].id);
        expect(warnings(logs)[0].message).toContain(warnings(logs)[0].id);
      }
    });

    test('the keys of `meta` are in the same order as without the cache', async () => {
      writeFiles(BASIC_FILES);
      const { clean, first, restart } = await buildThreeTimes<Record<string, string>>(
        (calls, seen) => [
          {
            name: 'first',
            resolveId: {
              cache: true,
              handler(source, importer) {
                if (!importer?.startsWith(src) || source !== './c.js') return null;
                calls.push(`resolveId:${rel(importer)}:${source}`);
                return { id: path.join(src, 'c.js'), meta: { key: 'resolveId', tail: 1 } };
              },
            },
            transform: {
              cache: true,
              handler(_code, id) {
                switch (id.startsWith(src) ? rel(id) : '') {
                  case 'a.js':
                    calls.push('transform:a.js');
                    return { meta: { shared: 'first', other: 1 } };
                  case 'c.js':
                    calls.push('transform:c.js');
                    // `key` is already set by `resolveId` of the importer `a.js`.
                    return { meta: { added: 2, key: 'transform' } };
                  default:
                    return null;
                }
              },
            },
          },
          {
            // `shared` is already set by the plugin `first`.
            name: 'second',
            transform: {
              cache: true,
              filter: { id: /[\\/]a\.js$/ },
              handler() {
                return { meta: { last: 3, shared: 'second' } };
              },
            },
          },
          {
            name: 'reader',
            buildEnd() {
              for (const name of ['a.js', 'c.js']) {
                seen[`buildEnd:${name}`] = JSON.stringify(
                  this.getModuleInfo(path.join(src, name))!.meta,
                );
              }
            },
            renderChunk(_code, chunk) {
              for (const id of chunk.moduleIds) {
                if (id.startsWith(src)) {
                  seen[`renderChunk:${rel(id)}`] = Object.keys(this.getModuleInfo(id)!.meta).join();
                }
              }
            },
          },
        ],
      );

      // A replaced key keeps its place.
      expect(clean.seen).toEqual({
        'buildEnd:a.js': '{"shared":"second","other":1,"last":3}',
        'buildEnd:c.js': '{"key":"transform","tail":1,"added":2}',
        'renderChunk:main.js': '',
        'renderChunk:a.js': 'shared,other,last',
        'renderChunk:b.js': '',
        'renderChunk:c.js': 'key,tail,added',
      });
      expect(restart.calls).toEqual([]);
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
    });

    test('a `meta` key with the value `undefined` replaces an older value', async () => {
      writeFiles(BASIC_FILES);
      type Flag = { flag: unknown; hasFlag: boolean; keys: string[] };
      const { clean, first, restart } = await buildThreeTimes<Record<string, Flag>>(
        (calls, seen) => [
          {
            name: 'sets',
            resolveId: {
              cache: true,
              handler(source, importer) {
                if (!importer?.startsWith(src) || source !== './c.js') return null;
                calls.push(`resolveId:${rel(importer)}:${source}`);
                return { id: path.join(src, 'c.js'), meta: { flag: 'resolveId' } };
              },
            },
            transform: {
              cache: true,
              filter: { id: /[\\/]a\.js$/ },
              handler(_code, id) {
                calls.push(`transform:${rel(id)}`);
                return { meta: { flag: true, kept: 1 } };
              },
            },
          },
          {
            name: 'clears',
            transform: {
              cache: true,
              filter: { id: /[\\/][ac]\.js$/ },
              handler() {
                return { meta: { flag: undefined } };
              },
            },
          },
          {
            name: 'reader',
            generateBundle() {
              for (const name of ['a.js', 'b.js', 'c.js']) {
                const meta = this.getModuleInfo(path.join(src, name))!.meta;
                seen[name] = {
                  flag: meta.flag === undefined ? '<undefined>' : meta.flag,
                  hasFlag: 'flag' in meta,
                  keys: Object.keys(meta),
                };
              }
            },
          },
        ],
      );

      expect(clean.seen).toEqual({
        'a.js': { flag: '<undefined>', hasFlag: true, keys: ['flag', 'kept'] },
        'b.js': { flag: '<undefined>', hasFlag: false, keys: [] },
        'c.js': { flag: '<undefined>', hasFlag: true, keys: ['flag'] },
      });
      expect(restart.calls).toEqual([]);
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
    });

    test('a `meta` that would not be the same after a restart gives no saved result', async () => {
      class Icons {
        list = ['home'];
        get count() {
          return this.list.length;
        }
      }
      const withoutPrototype = () => Object.assign(Object.create(null), { home: 1 });
      // For each module: the `meta` that its `load` hook returns, and what a later hook reads.
      const cases: Record<string, { meta: () => any; read: (meta: any) => unknown }> = {
        'class.js': {
          meta: () => new Icons(),
          read: (meta) => [meta instanceof Icons, meta.count],
        },
        'map.js': { meta: () => new Map([['icons', 1]]), read: (meta) => meta instanceof Map },
        'array.js': { meta: () => ['home'], read: (meta) => [Array.isArray(meta), meta[0]] },
        'no-prototype.js': {
          meta: withoutPrototype,
          read: (meta) => ['toString' in meta, meta.home],
        },
        'nested-no-prototype.js': {
          meta: () => ({ dict: withoutPrototype() }),
          read: (meta) => ['toString' in meta.dict, meta.dict.home],
        },
        'used-two-times.js': {
          meta: () => {
            const icon = { name: 'home' };
            return { first: icon, list: [icon] };
          },
          read: (meta) => meta.first === meta.list[0],
        },
        // One half of a surrogate pair. Rust cannot hold it in a string.
        'half-pair-key.js': {
          meta: () => ({ '\ud800x': 1 }),
          read: (meta) => Object.keys(meta).map((key) => key.charCodeAt(0)),
        },
        'plain.js': { meta: () => ({ icons: ['home'] }), read: (meta) => meta.icons },
      };
      const names = Object.keys(cases);
      writeFiles({
        'main.js': names.map((name) => `import './${name}';\n`).join(''),
        ...Object.fromEntries(names.map((name) => [name, `console.log('${name}');\n`])),
      });
      const { clean, first, restart } = await buildThreeTimes<Record<string, unknown>>(
        (calls, seen) => [
          {
            name: 'shapes',
            load: {
              cache: true,
              handler(id) {
                const name = id.startsWith(src) ? rel(id) : '';
                if (!(name in cases)) return null;
                calls.push(name);
                return { code: fs.readFileSync(id, 'utf8'), meta: cases[name].meta() };
              },
            },
          },
          {
            name: 'reader',
            buildEnd() {
              for (const name of names) {
                seen[name] = cases[name].read(this.getModuleInfo(path.join(src, name))!.meta);
              }
            },
          },
        ],
      );

      expect(clean.seen).toEqual({
        'class.js': [true, 1],
        'map.js': true,
        'array.js': [true, 'home'],
        'no-prototype.js': [false, 1],
        'nested-no-prototype.js': [false, 1],
        'used-two-times.js': true,
        'half-pair-key.js': [0xd800],
        'plain.js': ['home'],
      });
      // Only `plain.js` and `main.js` have a saved result.
      expect(restart.calls.sort()).toEqual(names.filter((name) => name !== 'plain.js').sort());
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
      expect(restart.output).toEqual(clean.output);

      const warnings = (logs: RolldownLog[]) =>
        logs.filter((log) => log.code === 'PERSISTENT_CACHE_UNSAVABLE_META');
      expect(warnings(clean.logs)).toEqual([]);
      for (const logs of [first.logs, restart.logs]) {
        expect(warnings(logs)).toHaveLength(1);
        expect(warnings(logs)[0]).toMatchObject({ plugin: 'shapes', hook: 'load' });
      }
    });

    /**
     * The module info of `id`, for a hook of another module. Rolldown knows a module when its
     * task starts, and the tasks of two imports of one module start in any order.
     */
    async function waitForModuleInfo(ctx: PluginContext, id: string) {
      for (let i = 0; i < 400; i++) {
        const info = ctx.getModuleInfo(id);
        if (info) return info;
        await sleep(5);
      }
      throw new Error(`no module info for ${id}`);
    }

    /** The part of `seen` from `hooks`. */
    const seenIn = (seen: Seen, ...hooks: string[]) =>
      Object.fromEntries(
        Object.entries(seen).filter(([key]) => hooks.some((hook) => key.startsWith(`${hook}:`))),
      );
    /** `seen` without the hooks that run while other modules are still loaded. */
    const seenAfterScan = (seen: Seen) => seenIn(seen, 'renderChunk', 'generateBundle');

    test('a hook that changes `meta` through `this.getModuleInfo` gets no saved result', async () => {
      writeFiles({
        'main.js': `import { a } from './a.js';\nimport { b } from './b.js';\nconsole.log(a, b);\n`,
        'a.js': `import { c } from './c.js';\nexport const a = 'a' + c;\n`,
        'b.js': `export const b = 'b';\n`,
        'c.js': `export const c = 'c';\n`,
      });
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        {
          name: 'base',
          transform: {
            cache: true,
            filter: { id: /[\\/]main\.js$/ },
            handler() {
              calls.push('base.transform:main.js');
              return { meta: { removed: 1, readOnly: { list: [1] }, kept: 'kept' } };
            },
          },
        },
        {
          name: 'writer',
          resolveId: {
            cache: true,
            handler(source, importer) {
              if (source === './c.js' && importer) {
                calls.push(`writer.resolveId:${rel(importer)}`);
                this.getModuleInfo(importer)!.meta.resolved = [source];
              }
              return null;
            },
          },
          load: {
            cache: true,
            filter: { id: /[\\/]a\.js$/ },
            handler(id) {
              calls.push(`writer.load:${rel(id)}`);
              const meta = this.getModuleInfo(id)!.meta;
              meta.loaded = { by: 'load' };
              meta.moved = 2;
              return null;
            },
          },
          transform: {
            cache: true,
            filter: { id: /[\\/]a\.js$/ },
            async handler(_code, id) {
              calls.push(`writer.transform:${rel(id)}`);
              // Its own meta. The `load` hook of this module task set `loaded` and `moved`.
              const own = this.getModuleInfo(id)!;
              own.meta.own = { from: rel(id) };
              own.meta.loaded.by += ' and transform';
              delete own.meta.moved;
              own.meta.moved = 3;
              // The meta of another module. `main.js` already ran its hooks.
              const main = this.getModuleInfo(path.join(src, 'main.js'))!.meta;
              main.fromA = ['a.js'];
              delete main.removed;
              if (main.readOnly.list.length !== 1) throw new Error('unexpected `readOnly`');
              const other = await waitForModuleInfo(this, path.join(src, 'b.js'));
              other.meta.fromA = ['home'];
              other.meta.fromA.push('search');
              return null;
            },
          },
        },
        readerPlugin(seen),
      ]);

      expect(seenIn(clean.seen, 'generateBundle')).toEqual({
        'generateBundle:main.js': {
          meta: { readOnly: { list: [1] }, kept: 'kept', fromA: ['a.js'] },
          moduleSideEffects: null,
        },
        'generateBundle:a.js': {
          meta: {
            loaded: { by: 'load and transform' },
            own: { from: 'a.js' },
            moved: 3,
            resolved: ['./c.js'],
          },
          moduleSideEffects: null,
        },
        'generateBundle:b.js': { meta: { fromA: ['home', 'search'] }, moduleSideEffects: null },
        'generateBundle:c.js': { meta: {}, moduleSideEffects: null },
      });
      expect(Object.keys(clean.seen)).toContain('renderChunk:main.js');
      // The keys are in the same order too.
      const keys = (seen: Seen) => Object.keys((seen['generateBundle:a.js']?.meta ?? {}) as object);
      expect(keys(clean.seen)).toEqual(['loaded', 'own', 'moved', 'resolved']);
      expect(keys(restart.seen)).toEqual(keys(clean.seen));

      expect(first.calls.sort()).toEqual(clean.calls.sort());
      // `main.js` keeps its saved result: its hook only returns `meta`. All hooks of `a.js` run
      // again. The other modules have no hook that runs.
      expect(restart.calls.sort()).toEqual([
        'writer.load:a.js',
        'writer.resolveId:a.js',
        'writer.transform:a.js',
      ]);
      expect(seenAfterScan(first.seen)).toEqual(seenAfterScan(clean.seen));
      expect(seenAfterScan(restart.seen)).toEqual(seenAfterScan(clean.seen));
      expect(first.output).toEqual(clean.output);
      expect(restart.output).toEqual(clean.output);

      expect(moduleInfoWarnings(clean.logs)).toEqual([]);
      for (const run of [first, restart]) {
        // `load` is the first hook of the module task that uses `meta`. After it, the task gets
        // no saved result, and the later hooks of the task give no warning.
        expect(moduleInfoWarnings(run.logs)).toEqual([
          expect.objectContaining({ plugin: 'writer', hook: 'load', id: path.join(src, 'a.js') }),
        ]);
      }
    });

    test('a hook that deletes a `meta` key of another module gets no saved result', async () => {
      writeFiles(BASIC_FILES);
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        {
          name: 'deleter',
          load: {
            cache: true,
            handler(id) {
              if (rel(id) !== 'b.js') return null;
              calls.push('load:b.js');
              return { code: fs.readFileSync(id, 'utf8'), meta: { x: 1 } };
            },
          },
          transform: {
            cache: true,
            async handler(_code, id) {
              if (rel(id) !== 'a.js') return null;
              calls.push('transform:a.js');
              for (let i = 0; i < 400; i++) {
                const info = this.getModuleInfo(path.join(src, 'b.js'));
                if (info && 'x' in info.meta) {
                  delete info.meta.x;
                  return null;
                }
                await sleep(5);
              }
              throw new Error('`b.js` never got `x`');
            },
          },
        },
        readerPlugin(seen),
      ]);

      expect(seenIn(clean.seen, 'generateBundle')['generateBundle:b.js']?.meta).toEqual({});
      expect(restart.calls).toEqual(['transform:a.js']);
      expect(seenAfterScan(first.seen)).toEqual(seenAfterScan(clean.seen));
      expect(seenAfterScan(restart.seen)).toEqual(seenAfterScan(clean.seen));
      expect(restart.output).toEqual(clean.output);
    });

    test('a nested `this.resolve` call that uses `meta` names the module of the outer hook', async () => {
      writeFiles(BASIC_FILES);
      const main = path.join(src, 'main.js');
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        {
          name: 'outer',
          transform: {
            cache: true,
            async handler(_code, id) {
              if (id === main) return { meta: { x: 1 } };
              if (rel(id) !== 'a.js') return null;
              calls.push('outer.transform:a.js');
              // The importer is another module. The nested hook still belongs to `a.js`.
              await this.resolve('virtual:x', path.join(src, 'b.js'));
              return null;
            },
          },
        },
        {
          name: 'inner',
          resolveId: {
            cache: true,
            async handler(source, importer) {
              if (source !== 'virtual:x') return null;
              calls.push(`inner.resolveId:${rel(importer!)}:${source}`);
              if ((await waitForModuleInfo(this, main)).meta.x !== 1) {
                throw new Error('unexpected `meta` of `main.js`');
              }
              return '\0x';
            },
          },
        },
        readerPlugin(seen),
      ]);

      expect(restart.calls.sort()).toEqual([
        'inner.resolveId:b.js:virtual:x',
        'outer.transform:a.js',
      ]);
      expect(seenAfterScan(restart.seen)).toEqual(seenAfterScan(clean.seen));
      expect(restart.output).toEqual(clean.output);

      expect(moduleInfoWarnings(clean.logs)).toEqual([]);
      for (const run of [first, restart]) {
        const warnings = moduleInfoWarnings(run.logs);
        expect(warnings).toEqual([
          expect.objectContaining({
            plugin: 'inner',
            hook: 'resolveId',
            id: path.join(src, 'a.js'),
          }),
        ]);
        expect(warnings[0].message).toContain(`the \`meta\` of "${main}"`);
        expect(warnings[0].message).toContain(`"${path.join(src, 'a.js')}" gets no saved result`);
      }
    });

    /** Copies the `meta` of `main.js` in `buildEnd`, a hook that the cache never skips. */
    function mainMetaReader(seen: { meta?: unknown }): Plugin {
      return {
        name: 'main-meta-reader',
        buildEnd() {
          seen.meta = structuredClone(this.getModuleInfo(path.join(src, 'main.js'))!.meta);
        },
      };
    }

    describe('a hook changes a `meta` value that another module task set', () => {
      // The hook changes `meta` through `this.getModuleInfo`. So Rolldown saves no result for
      // the module task of such a hook call, and the hooks run again.
      const collector = (
        calls: string[],
        push: (ctx: PluginContext, code: string, id: string) => void | Promise<void>,
      ): Plugin => ({
        name: 'collector',
        transform: {
          cache: true,
          async handler(code, id) {
            if (!id.startsWith(src)) return null;
            calls.push(rel(id));
            if (rel(id) === 'main.js') return { code, meta: { list: [] } };
            await push(this, code, id);
            return null;
          },
        },
      });

      test('an edited module leaves no old item behind', async () => {
        // main.js -> a.js -> b.js. So the hooks run in this order with and without the cache.
        writeFiles({
          'main.js': `import './a.js';\nconsole.log('main');\n`,
          'a.js': `/* PUSH */\nimport './b.js';\nconsole.log('a');\n`,
          'b.js': `/* PUSH */\nconsole.log('b');\n`,
        });
        const makePlugins = (calls: string[], seen: { meta?: unknown }): Plugin[] => [
          collector(calls, (ctx, code, id) => {
            if (code.includes('PUSH')) {
              ctx.getModuleInfo(path.join(src, 'main.js'))!.meta.list.push(rel(id));
            }
          }),
          mainMetaReader(seen),
        ];

        await build({ plugins: makePlugins([], {}) });

        // `a.js` no longer pushes. It still imports `b.js`.
        writeFiles({ 'a.js': `import './b.js';\nconsole.log('a');\n` });

        const clean: { meta?: unknown } = {};
        await build({ plugins: makePlugins([], clean), cache: undefined });
        expect(clean.meta).toEqual({ list: ['b.js'] });

        const calls: string[] = [];
        const restart: { meta?: unknown } = {};
        await build({ plugins: makePlugins(calls, restart) });
        // `a.js` changed. `b.js` has no saved result: a saved result would hold the whole list
        // `['a.js', 'b.js']`.
        expect(calls).toEqual(['a.js', 'b.js']);
        expect(restart.meta).toEqual(clean.meta);
      });

      test('a restart without a change loses no item', async () => {
        writeFiles({
          'main.js': `import './a.js';\nimport './b.js';\nconsole.log('main');\n`,
          'a.js': `console.log('a');\n`,
          // A large file: the cache needs more time to make its key. A saved result of `b.js`
          // would be used after the one of `a.js`.
          'b.js': `/* ${'x'.repeat(40 * 1024 * 1024)} */\nconsole.log('b');\n`,
        });
        const makePlugins = (calls: string[], seen: { meta?: unknown }): Plugin[] => {
          let bDone!: () => void;
          const bIsDone = new Promise<void>((resolve) => (bDone = resolve));
          return [
            collector(calls, async (ctx, _code, id) => {
              // The hook call of `b.js` ends first, then the one of `a.js`.
              if (rel(id) === 'a.js') {
                await bIsDone;
                await sleep(50);
              }
              ctx.getModuleInfo(path.join(src, 'main.js'))!.meta.list.push(rel(id));
              if (rel(id) === 'b.js') bDone();
            }),
            mainMetaReader(seen),
          ];
        };

        const clean: { meta?: unknown } = {};
        await build({ plugins: makePlugins([], clean), cache: undefined });
        expect(clean.meta).toEqual({ list: ['b.js', 'a.js'] });

        const first: { meta?: unknown } = {};
        await build({ plugins: makePlugins([], first) });
        expect(first.meta).toEqual(clean.meta);

        const calls: string[] = [];
        const restart: { meta?: unknown } = {};
        await build({ plugins: makePlugins(calls, restart) });
        // A saved result of `b.js` would hold `['b.js']`, and it would be written last.
        expect(calls.sort()).toEqual(['a.js', 'b.js']);
        expect(restart.meta).toEqual(clean.meta);
      });

      test('a new value that is made from the old one', async () => {
        writeFiles({
          'main.js': `import './a.js';\nconsole.log('main');\n`,
          'a.js': `import './b.js';\nconsole.log('a');\n`,
          'b.js': `console.log('b');\n`,
        });
        const makePlugins = (calls: string[], seen: { meta?: unknown }): Plugin[] => [
          {
            name: 'counter',
            transform: {
              cache: true,
              handler(code, id) {
                if (!id.startsWith(src)) return null;
                calls.push(rel(id));
                if (rel(id) === 'main.js') return { code, meta: { count: 0 } };
                const meta = this.getModuleInfo(path.join(src, 'main.js'))!.meta;
                meta.count = meta.count + (code.includes('TWO') ? 2 : 1);
                return null;
              },
            },
          },
          mainMetaReader(seen),
        ];
        await build({ plugins: makePlugins([], {}) });
        writeFiles({ 'a.js': `/* TWO */\nimport './b.js';\nconsole.log('a');\n` });

        const clean: { meta?: unknown } = {};
        await build({ plugins: makePlugins([], clean), cache: undefined });
        expect(clean.meta).toEqual({ count: 3 });

        const calls: string[] = [];
        const restart: { meta?: unknown } = {};
        await build({ plugins: makePlugins(calls, restart) });
        expect(calls).toEqual(['a.js', 'b.js']);
        expect(restart.meta).toEqual(clean.meta);
      });
    });

    test('two module tasks set the same key of a third module through `this.getModuleInfo`', async () => {
      // main.js -> a.js -> b.js. Both `a.js` and `b.js` set `flag` on `main.js`.
      const files = {
        'main.js': `import './a.js';\nconsole.log('main');\n`,
        'a.js': `/* FLAG */\nimport './b.js';\nconsole.log('a');\n`,
        'b.js': `/* FLAG */\nconsole.log('b');\n`,
      };
      writeFiles(files);
      const makePlugins = (calls: string[], seen: { meta?: unknown }): Plugin[] => [
        {
          name: 'flags',
          transform: {
            cache: true,
            handler(code, id) {
              if (!id.startsWith(src)) return null;
              calls.push(rel(id));
              if (code.includes('FLAG')) {
                this.getModuleInfo(path.join(src, 'main.js'))!.meta.flag = true;
              }
              return null;
            },
          },
        },
        mainMetaReader(seen),
      ];
      const run = async (cache: CacheOption) => {
        const calls: string[] = [];
        const seen: { meta?: unknown } = {};
        await build({ plugins: makePlugins(calls, seen), cache });
        return { calls, meta: seen.meta };
      };

      await run({ dir: cacheDir });
      // Both hooks write `meta` through `this.getModuleInfo`, so neither module has a saved
      // result. `main.js` has one.
      expect(await run({ dir: cacheDir })).toEqual({
        calls: ['a.js', 'b.js'],
        meta: { flag: true },
      });

      // `a.js` no longer sets the flag. `b.js` still does.
      writeFiles({ 'a.js': `import './b.js';\nconsole.log('a');\n` });
      const clean = await run(undefined);
      expect(clean.meta).toEqual({ flag: true });
      expect(await run({ dir: cacheDir })).toEqual({ calls: ['a.js', 'b.js'], meta: clean.meta });

      // Now `a.js` does not use `meta`, so it has a saved result.
      expect(await run({ dir: cacheDir })).toEqual({ calls: ['b.js'], meta: clean.meta });

      // `b.js` no longer sets the flag either.
      writeFiles({ 'b.js': `console.log('b');\n` });
      expect((await run(undefined)).meta).toEqual({});
      expect(await run({ dir: cacheDir })).toEqual({ calls: ['b.js'], meta: {} });
      expect(await run({ dir: cacheDir })).toEqual({ calls: [], meta: {} });
    });

    test('the `meta` in a hook that the cache records is the plain object of the module', async () => {
      writeFiles({ 'main.js': `console.log('main');\n` });
      const makePlugins = (seen: Record<string, unknown>): Plugin[] => {
        const byMeta = new WeakMap<object, string>();
        return [
          {
            name: 'keyed-by-meta',
            transform: {
              cache: true,
              handler(_code, id) {
                if (!id.startsWith(src)) return null;
                const meta = this.getModuleInfo(id)!.meta;
                byMeta.set(meta, 'data');
                seen.sameInOneCall = this.getModuleInfo(id)!.meta === meta;
                seen.clone = structuredClone(meta);
                seen.isPlain = Object.getPrototypeOf(meta) === Object.prototype;
                return null;
              },
            },
            buildEnd() {
              seen.foundInBuildEnd = byMeta.has(
                this.getModuleInfo(path.join(src, 'main.js'))!.meta,
              );
            },
          },
        ];
      };
      const clean: Record<string, unknown> = {};
      await build({ plugins: makePlugins(clean), cache: undefined });
      expect(clean).toEqual({
        sameInOneCall: true,
        clone: {},
        isPlain: true,
        foundInBuildEnd: true,
      });
      // The first build with the cache: the hook runs, as without the cache.
      const first: Record<string, unknown> = {};
      await build({ plugins: makePlugins(first) });
      expect(first).toEqual(clean);
    });

    test('a hook that only reads `meta` gets no saved result, and other module info does not count', async () => {
      writeFiles(BASIC_FILES);
      const { clean, first, restart } = await buildThreeTimes<Record<string, unknown>>(
        (calls, seen) => [
          {
            name: 'kinds',
            resolveId: {
              cache: true,
              handler(source, importer) {
                // `a.js` imports `./c.js`, so the task of `a.js` sets this key of `c.js`.
                return importer && source === './c.js'
                  ? { id: path.join(src, 'c.js'), meta: { kind: 'leaf' } }
                  : null;
              },
            },
            transform: {
              cache: true,
              async handler(code, id) {
                if (!id.startsWith(src)) return null;
                calls.push(rel(id));
                const info = this.getModuleInfo(id)!;
                switch (rel(id)) {
                  case 'main.js':
                    // Not a use of `meta`.
                    seen['isEntry:main.js'] = info.isEntry;
                    break;
                  case 'a.js':
                    seen['own:a.js'] = structuredClone(info.meta);
                    break;
                  case 'b.js': {
                    // Not a use of `meta` either.
                    const resolved = await this.resolve('./a.js', path.join(src, 'main.js'));
                    seen['resolved:a.js'] = rel(resolved!.id);
                    break;
                  }
                  case 'c.js': {
                    const resolved = await this.resolve('./c.js', path.join(src, 'a.js'));
                    seen['resolved:c.js'] = { ...resolved!.meta };
                    break;
                  }
                }
                return code.replaceAll('__NAME__', JSON.stringify(rel(id)));
              },
            },
          },
        ],
      );
      expect(clean.seen).toEqual({
        'isEntry:main.js': true,
        'own:a.js': {},
        'resolved:a.js': 'a.js',
        'resolved:c.js': { kind: 'leaf' },
      });
      expect(first.seen).toEqual(clean.seen);
      expect(restart.calls.sort()).toEqual(['a.js', 'c.js']);
      expect(restart.output).toEqual(clean.output);
      for (const run of [first, restart]) {
        expect(moduleInfoWarnings(run.logs)).toHaveLength(1);
        expect(moduleInfoWarnings(run.logs)[0]).toMatchObject({
          plugin: 'kinds',
          hook: 'transform',
        });
      }
    });

    test('a change to the `meta` of a `this.resolve` result gives no saved result', async () => {
      writeFiles({
        'main.js': `import './a.js';\nconsole.log('main');\n`,
        'a.js': `console.log('a');\n`,
      });
      const { clean, first, restart } = await buildThreeTimes<{ meta?: unknown }>((calls, seen) => [
        {
          name: 'resolver',
          transform: {
            cache: true,
            async handler(_code, id) {
              if (rel(id) !== 'a.js') return null;
              calls.push(rel(id));
              const resolved = await this.resolve(path.join(src, 'main.js'));
              resolved!.meta.fromResolve = true;
              return null;
            },
          },
        },
        mainMetaReader(seen),
      ]);
      expect(clean.seen.meta).toEqual({ fromResolve: true });
      expect(first.seen).toEqual(clean.seen);
      expect(restart.calls).toEqual(['a.js']);
      expect(restart.seen).toEqual(clean.seen);
      expect(restart.output).toEqual(clean.output);
    });

    test.each([
      { name: 'as it is', reused: true },
      // A spread reads `meta`, so the hook gets no saved result.
      { name: 'spread', reused: false },
      { name: 'spread, with a new `meta`', reused: false },
    ])('a hook that returns a `this.resolve` result $name', async ({ name, reused }) => {
      // `main.js` imports `c.js` first, so the hooks of `c.js` can end before `a.js` resolves it.
      writeFiles({
        'main.js': `import './c.js';\nimport './a.js';\nconsole.log('main');\n`,
        'a.js': `import './c.js';\nconsole.log('a');\n`,
        'c.js': `/* key: one */\nconsole.log('c');\n`,
      });
      const makePlugins = (calls: string[], seen: Seen): Plugin[] => {
        let cDone!: () => void;
        const cIsDone = new Promise<void>((resolve) => (cDone = resolve));
        return [
          {
            name: 'keys',
            transform: {
              cache: true,
              filter: { id: /[\\/]c\.js$/ },
              handler(code) {
                calls.push('transform:c.js');
                cDone();
                return { meta: { [/key: (\w+)/.exec(code)![1]]: true } };
              },
            },
          },
          {
            name: 'pass-on',
            resolveId: {
              cache: true,
              async handler(source, importer) {
                if (source !== './c.js' || !importer?.endsWith('a.js')) return null;
                calls.push('resolveId:a.js');
                await cIsDone;
                await sleep(20);
                // The result holds the `meta` object of `c.js`. Rolldown reads it after the hook
                // call ended, so returning it as it is does not count as a use of `meta`.
                const resolved = (await this.resolve(source, importer, { skipSelf: true }))!;
                if (name === 'spread') return { ...resolved };
                if (name === 'spread, with a new `meta`') {
                  return { ...resolved, meta: { ...resolved.meta, own: 1 } };
                }
                return resolved;
              },
            },
          },
          readerPlugin(seen),
        ];
      };
      await build({ plugins: makePlugins([], {}) });
      writeFiles({ 'c.js': `/* key: two */\nconsole.log('c');\n` });

      const cleanSeen: Seen = {};
      const cleanOutput = await build({ plugins: makePlugins([], cleanSeen), cache: undefined });
      expect(cleanSeen['generateBundle:c.js']?.meta).toEqual(
        name === 'spread, with a new `meta`' ? { two: true, own: 1 } : { two: true },
      );

      const calls: string[] = [];
      const seen: Seen = {};
      const output = await build({ plugins: makePlugins(calls, seen) });
      // A saved result of `a.js` would not hold the key `one` of `c.js`.
      expect(calls.sort()).toEqual(
        reused ? ['transform:c.js'] : ['resolveId:a.js', 'transform:c.js'],
      );
      expect(seenAfterScan(seen)).toEqual(seenAfterScan(cleanSeen));
      expect(output).toEqual(cleanOutput);
    });

    test('a `resolveDynamicImport` hook that assigns `moduleSideEffects` gets no saved result', async () => {
      writeFiles({
        'main.js': `import { a } from './a.js';\nimport './side.js';\nconsole.log(a);\n`,
        'a.js': `export const a = () => import('./lazy.js');\n`,
        'side.js': `console.log('side effect of side');\n`,
        'lazy.js': `export const lazy = 'lazy';\n`,
      });
      const { clean, first, restart } = await buildThreeTimes(
        (calls, seen) => [
          {
            name: 'dynamic',
            resolveDynamicImport: {
              cache: true,
              async handler(source, importer) {
                if (source !== './lazy.js' || !importer) return null;
                calls.push(`resolveDynamicImport:${rel(importer)}`);
                // `side.js` is another import of `main.js`, so its task has started.
                const side = await waitForModuleInfo(this, path.join(src, 'side.js'));
                side.moduleSideEffects = true;
                return null;
              },
            },
          },
          readerPlugin(seen),
        ],
        { treeshake: { moduleSideEffects: false } },
      );

      expect(clean.calls).toEqual(['resolveDynamicImport:a.js']);
      expect(seenIn(clean.seen, 'generateBundle')).toMatchObject({
        'generateBundle:side.js': { meta: {}, moduleSideEffects: true },
      });
      // Only the assignment keeps `side.js`.
      const code = clean.output.map((file) => (file.type === 'chunk' ? file.code : '')).join('\n');
      expect(code).toContain('side effect of side');

      expect(restart.calls).toEqual(['resolveDynamicImport:a.js']);
      expect(seenAfterScan(first.seen)).toEqual(seenAfterScan(clean.seen));
      expect(seenAfterScan(restart.seen)).toEqual(seenAfterScan(clean.seen));
      expect(first.output).toEqual(clean.output);
      expect(restart.output).toEqual(clean.output);
      expect(moduleInfoWarnings(clean.logs)).toEqual([]);
      for (const run of [first, restart]) {
        expect(moduleInfoWarnings(run.logs)).toEqual([
          expect.objectContaining({
            plugin: 'dynamic',
            hook: 'resolveDynamicImport',
            id: path.join(src, 'a.js'),
          }),
        ]);
      }
    });

    test('a use of `meta` through `this.getModuleInfo` gives one warning, apart from other cache warnings', async () => {
      writeFiles(BASIC_FILES);
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        {
          name: 'writer',
          transform: {
            cache: true,
            handler(code, id) {
              if (!id.startsWith(src)) return null;
              calls.push(rel(id));
              code = code.replaceAll('__NAME__', JSON.stringify(rel(id)));
              switch (rel(id)) {
                // Two modules that use `meta`, and still one warning.
                case 'a.js':
                case 'b.js':
                  this.getModuleInfo(id)!.meta.icons = [rel(id)];
                  return code;
                // The same plugin and hook, with a warning of another kind.
                case 'c.js':
                  return { code, meta: { icons: new Set([rel(id)]) } };
                default:
                  return code;
              }
            },
          },
        },
        readerPlugin(seen),
      ]);

      const unsavable = (logs: RolldownLog[]) =>
        logs.filter((log) => log.code === 'PERSISTENT_CACHE_UNSAVABLE_META');
      expect(moduleInfoWarnings(clean.logs)).toEqual([]);
      expect(unsavable(clean.logs)).toEqual([]);
      for (const run of [first, restart]) {
        const [warning, ...more] = moduleInfoWarnings(run.logs);
        expect(more).toEqual([]);
        expect(warning).toMatchObject({ plugin: 'writer', hook: 'transform' });
        expect([path.join(src, 'a.js'), path.join(src, 'b.js')]).toContain(warning.id);
        for (const text of ['"writer"', '"transform"', warning.id!, 'this.getModuleInfo']) {
          expect(warning.message).toContain(text);
        }
        expect(warning.message).toContain('read or changed the `meta` of');
        expect(warning.message).toContain('return the `meta` from this hook');
        expect(unsavable(run.logs)).toEqual([
          expect.objectContaining({
            plugin: 'writer',
            hook: 'transform',
            id: path.join(src, 'c.js'),
          }),
        ]);
        expect(run.seen).toEqual(clean.seen);
        expect(run.output).toEqual(clean.output);
      }
      expect(clean.seen['generateBundle:a.js']?.meta).toEqual({ icons: ['a.js'] });
      expect(clean.seen['generateBundle:c.js']?.meta).toEqual({ icons: new Set(['c.js']) });
      expect(restart.calls.sort()).toEqual(['a.js', 'b.js', 'c.js']);
    });

    test.each([
      {
        name: '`true` keeps a module that `treeshake.moduleSideEffects: false` removes',
        treeshake: { moduleSideEffects: false },
        assigned: true,
        inOutput: true,
      },
      {
        name: '`false` removes a module that has side effects',
        treeshake: undefined,
        assigned: false,
        inOutput: false,
      },
      {
        name: '`no-treeshake` keeps a module that `treeshake.moduleSideEffects: false` removes',
        treeshake: { moduleSideEffects: false },
        assigned: 'no-treeshake',
        inOutput: true,
      },
      { name: '`null`', treeshake: undefined, assigned: null, inOutput: true },
    ] as const)(
      'an assignment to `moduleSideEffects` of another module gives no saved result: $name',
      async ({ treeshake, assigned, inOutput }) => {
        writeFiles({
          'main.js': [
            `import { a } from './a.js';`,
            `import './b.js';`,
            `import './c.js';`,
            `console.log(a);`,
            ``,
          ].join('\n'),
          'a.js': `export const a = 'a';\n`,
          'b.js': `console.log('side effect of b');\n`,
          'c.js': `console.log('side effect of c');\n`,
        });
        const { clean, first, restart } = await buildThreeTimes(
          (calls, seen) => [
            {
              name: 'assigner',
              load: {
                cache: true,
                handler(id) {
                  if (id.startsWith(src)) calls.push(`load:${rel(id)}`);
                  return null;
                },
              },
              transform: {
                cache: true,
                async handler(_code, id) {
                  if (!id.startsWith(src)) return null;
                  calls.push(`transform:${rel(id)}`);
                  if (rel(id) === 'a.js') {
                    const info = await waitForModuleInfo(this, path.join(src, 'b.js'));
                    info.moduleSideEffects = assigned;
                    if (info.moduleSideEffects !== assigned) throw new Error('not assigned');
                  }
                  return null;
                },
              },
            },
            readerPlugin(seen),
          ],
          { treeshake },
        );

        const code = clean.output
          .map((file) => (file.type === 'chunk' ? file.code : ''))
          .join('\n');
        expect(code.includes('side effect of b')).toBe(inOutput);
        // `c.js` has no assignment, so `treeshake.moduleSideEffects` alone decides.
        expect(code.includes('side effect of c')).toBe(treeshake === undefined);
        expect(clean.seen['generateBundle:b.js']).toEqual({
          meta: {},
          moduleSideEffects: assigned,
        });
        expect(clean.seen['generateBundle:c.js']).toEqual({ meta: {}, moduleSideEffects: null });

        // `b.js` keeps its saved result. The assignment is not part of it.
        expect(restart.calls).toEqual(['load:a.js', 'transform:a.js']);
        expect(seenAfterScan(first.seen)).toEqual(seenAfterScan(clean.seen));
        expect(seenAfterScan(restart.seen)).toEqual(seenAfterScan(clean.seen));
        expect(first.output).toEqual(clean.output);
        expect(restart.output).toEqual(clean.output);
        expect(moduleInfoWarnings(clean.logs)).toEqual([]);
        for (const run of [first, restart]) {
          const [warning, ...more] = moduleInfoWarnings(run.logs);
          expect(more).toEqual([]);
          expect(warning).toMatchObject({
            plugin: 'assigner',
            hook: 'transform',
            id: path.join(src, 'a.js'),
          });
          for (const text of ['"assigner"', '"transform"', warning.id!, 'this.getModuleInfo']) {
            expect(warning.message).toContain(text);
          }
          expect(warning.message).toContain('assigned the `moduleSideEffects` of');
        }
      },
    );

    test('an assignment to `moduleSideEffects` of the module itself gives no saved result', async () => {
      writeFiles(BASIC_FILES);
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        {
          name: 'assigner',
          transform: {
            cache: true,
            handler(code, id) {
              if (!id.startsWith(src)) return null;
              calls.push(rel(id));
              if (rel(id) === 'a.js') this.getModuleInfo(id)!.moduleSideEffects = 'no-treeshake';
              return code.replaceAll('__NAME__', JSON.stringify(rel(id)));
            },
          },
        },
        readerPlugin(seen),
      ]);
      expect(clean.seen['generateBundle:a.js']?.moduleSideEffects).toBe('no-treeshake');
      expect(restart.calls).toEqual(['a.js']);
      for (const run of [first, restart]) {
        expect(run.seen).toEqual(clean.seen);
        expect(run.output).toEqual(clean.output);
        expect(moduleInfoWarnings(run.logs)).toEqual([
          expect.objectContaining({ plugin: 'assigner', hook: 'transform' }),
        ]);
      }
    });

    test('an assignment after the hook of the module returned a value wins after an edit of the module', async () => {
      // Without the cache, the assignment comes after the hook of `b.js` returned `false`, so
      // `b.js` is kept. A saved assignment would come before the new result of `b.js`.
      writeFiles({
        'main.js': `import './a.js';\nimport './b.js';\nconsole.log('main');\n`,
        'a.js': `console.log('a');\n`,
        'b.js': `console.log('side effect of b');\n`,
      });
      const makePlugins = (calls: string[]): Plugin[] => [
        {
          name: 'side-effects',
          transform: {
            cache: true,
            async handler(code, id) {
              if (!id.startsWith(src)) return null;
              calls.push(`transform:${rel(id)}`);
              if (rel(id) === 'b.js') return { code, moduleSideEffects: false };
              if (rel(id) === 'a.js') {
                // Wait until `b.js` is parsed, so its hooks have ended.
                for (let i = 0; i < 400; i++) {
                  const info = this.getModuleInfo(path.join(src, 'b.js'));
                  if (info?.code != null) {
                    info.moduleSideEffects = true;
                    return null;
                  }
                  await sleep(5);
                }
                throw new Error('b.js is not parsed');
              }
              return null;
            },
          },
        },
      ];
      const options = { treeshake: { moduleSideEffects: false } };
      await build({ ...options, plugins: makePlugins([]) });
      writeFiles({ 'b.js': `console.log('side effect of b');\n// edited\n` });

      const cleanOutput = await build({ ...options, plugins: makePlugins([]), cache: undefined });
      const code = cleanOutput.map((file) => (file.type === 'chunk' ? file.code : '')).join('\n');
      expect(code).toContain('side effect of b');
      const calls: string[] = [];
      expect(await build({ ...options, plugins: makePlugins(calls) })).toEqual(cleanOutput);
      expect(calls.sort()).toEqual(['transform:a.js', 'transform:b.js']);
    });

    describe('a hook returns the `meta` object of another module', () => {
      test.each(['resolveId', 'resolveDynamicImport'] as const)(
        '`%s` returns a `this.resolve` result with another id',
        async (hook) => {
          // main.js -> b.js -> a.js. So the `transform` hook of `b.js` ends before `a.js` imports
          // `virtual`.
          const importVirtual = hook === 'resolveId' ? `import 'virtual';` : `import('virtual');`;
          writeFiles({
            'main.js': `import './b.js';\n`,
            'b.js': `import './a.js';\nconsole.log('b'); // tag:one\n`,
            'a.js': `${importVirtual}\n`,
            'c.js': `console.log('c');\n`,
          });
          const makePlugins = (calls: string[], seen: { meta?: unknown }): Plugin[] => {
            const resolveHook = {
              cache: true,
              async handler(this: PluginContext, source: string, importer: string | undefined) {
                if (source !== 'virtual') return null;
                calls.push(`${hook}:${rel(importer!)}`);
                const resolved = (await this.resolve('./b.js', importer))!;
                // `resolved.meta` is the `meta` object of `b.js`. The hook never reads it.
                resolved.id = path.join(src, 'c.js');
                return resolved;
              },
            };
            return [
              {
                name: 'tagger',
                transform: {
                  cache: true,
                  handler(code, id) {
                    const tag = /tag:(\w+)/.exec(code)?.[1];
                    return rel(id) === 'b.js' && tag ? { code, meta: { tag } } : null;
                  },
                },
              },
              { name: 'retarget', [hook]: resolveHook },
              {
                name: 'c-meta-reader',
                buildEnd() {
                  seen.meta = structuredClone(this.getModuleInfo(path.join(src, 'c.js'))!.meta);
                },
              },
            ];
          };
          const logs: RolldownLog[] = [];
          await build({
            plugins: makePlugins([], {}),
            onLog: (_level, log) => void logs.push(log),
          });
          const [warning, ...more] = moduleInfoWarnings(logs);
          expect(more).toEqual([]);
          expect(warning).toMatchObject({ plugin: 'retarget', hook, id: path.join(src, 'a.js') });
          expect(warning.message).toContain(
            'read or changed the `meta` of "' + path.join(src, 'b.js') + '"',
          );

          writeFiles({ 'b.js': `import './a.js';\nconsole.log('b'); // tag:two\n` });
          const clean: { meta?: unknown } = {};
          const cleanOutput = await build({ plugins: makePlugins([], clean), cache: undefined });
          // Without the cache, `c.js` shares the `meta` object of `b.js`.
          expect(clean.meta).toEqual({ tag: 'two' });

          const calls: string[] = [];
          const restart: { meta?: unknown } = {};
          const output = await build({ plugins: makePlugins(calls, restart) });
          // A saved result of `a.js` would give `c.js` the old `{ tag: 'one' }`.
          expect(calls).toEqual([`${hook}:a.js`]);
          expect(restart.meta).toEqual(clean.meta);
          expect(output).toEqual(cleanOutput);
        },
      );

      test('`resolveId` returns the `meta` object of the module itself', async () => {
        writeFiles({
          'main.js': `import './b.js';\n`,
          'b.js': `import './a.js';\nconsole.log('b'); // tag:one\n`,
          'a.js': `import 'virtual';\n`,
        });
        const { clean, first, restart } = await buildThreeTimes((calls) => [
          {
            name: 'tagger',
            transform: {
              cache: true,
              handler(code, id) {
                const tag = /tag:(\w+)/.exec(code)?.[1];
                return rel(id) === 'b.js' && tag ? { code, meta: { tag } } : null;
              },
            },
          },
          {
            name: 'same-module',
            resolveId: {
              cache: true,
              async handler(source, importer) {
                if (source !== 'virtual') return null;
                calls.push(`resolveId:${rel(importer!)}`);
                return this.resolve('./b.js', importer);
              },
            },
          },
        ]);
        // The hook call sets nothing, so the saved result of `a.js` is used.
        expect(restart.calls).toEqual([]);
        for (const run of [clean, first, restart]) {
          expect(moduleInfoWarnings(run.logs)).toEqual([]);
          expect(run.output).toEqual(clean.output);
        }
      });

      // Both orders end with the same values. In each order, the `transform` hook of `a.js` runs
      // after both resolve hook calls.
      test.each(['a.js', 'b.js'])(
        'one object is returned as the `meta` of two modules, %s first',
        async (firstModule) => {
          writeFiles({
            'main.js': `import './a.js';\nimport './b.js';\nconsole.log('main');\n`,
            'a.js': `console.log('a');\n`,
            'b.js': `console.log('b');\n`,
          });
          const { clean, first, restart } = await buildThreeTimes((calls, seen) => {
            const common = { tag: 1 };
            const resolved = new Map<string, () => void>();
            const isResolved = (name: string) =>
              new Promise<void>((resolve) => resolved.set(name, resolve));
            const done = { 'a.js': isResolved('a.js'), 'b.js': isResolved('b.js') };
            const waitFor = async (name: 'a.js' | 'b.js') => {
              await Promise.race([done[name], sleep(200)]);
              await sleep(20);
            };
            return [
              {
                name: 'common-meta',
                resolveId: {
                  cache: true,
                  async handler(source, importer) {
                    if (!importer?.startsWith(src) || !source.startsWith('./')) return null;
                    calls.push(`resolveId:${rel(importer)}:${source}`);
                    const name = path.basename(source);
                    if (name !== firstModule) await waitFor(firstModule as 'a.js' | 'b.js');
                    resolved.get(name)!();
                    return { id: path.join(src, source), meta: common };
                  },
                },
              },
              {
                name: 'extra-meta',
                transform: {
                  cache: true,
                  async handler(code, id) {
                    if (rel(id) !== 'a.js') return null;
                    calls.push('transform:a.js');
                    await waitFor('b.js');
                    return { code, meta: { extra: 'a' } };
                  },
                },
              },
              readerPlugin(seen),
            ];
          });
          // Without the cache, `a.js` and `b.js` share one `meta` object, so both get `extra`.
          expect(clean.seen['buildEnd:b.js']).toEqual({
            meta: { tag: 1, extra: 'a' },
            moduleSideEffects: null,
          });
          expect(moduleInfoWarnings(clean.logs)).toEqual([]);
          // A saved result of `main.js` or `a.js` would give `b.js` only `{ tag: 1 }`.
          expect(restart.calls.sort()).toEqual([
            'resolveId:main.js:./a.js',
            'resolveId:main.js:./b.js',
            'transform:a.js',
          ]);
          for (const run of [first, restart]) {
            expect(run.seen).toEqual(clean.seen);
            expect(run.output).toEqual(clean.output);
            expect(moduleInfoWarnings(run.logs)).toEqual([
              expect.objectContaining({
                plugin: 'common-meta',
                hook: 'resolveId',
                id: path.join(src, 'main.js'),
              }),
              expect.objectContaining({
                plugin: 'extra-meta',
                hook: 'transform',
                id: path.join(src, 'a.js'),
              }),
            ]);
          }
        },
      );
    });

    describe('a hook uses `moduleSideEffects` that another module task set', () => {
      // `warning` is the use that the warning names. Passing on the value gives no warning.
      test.each([
        // The wrapper passes on the value that `b.js` already has.
        { name: 'an object without `moduleSideEffects`', warning: undefined, reused: false },
        { name: 'a `this.resolve` result as it is', warning: undefined, reused: false },
        // A spread reads `meta` before `moduleSideEffects`, so the first use is a use of `meta`.
        { name: 'a spread of a `this.resolve` result', warning: 'meta', reused: false },
        { name: 'its own object with the read value', warning: 'read', reused: false },
        { name: 'a `this.resolve` result with a new value', warning: undefined, reused: true },
        {
          name: 'a `this.resolve` result as it is, from `resolveDynamicImport`',
          warning: undefined,
          reused: false,
        },
      ] as const)('the resolve hook returns $name', async ({ name, warning: use, reused }) => {
        const dynamic = name.includes('resolveDynamicImport');
        const importB = dynamic ? `import('./b.js');` : `import './b.js';`;
        // `main.js` imports `b.js` first, so the hooks of `b.js` can end before `a.js` resolves it.
        writeFiles({
          'main.js': `${importB}\nimport './a.js';\nconsole.log('main');\n`,
          'a.js': `${importB}\nconsole.log('a');\n`,
          'b.js': `/* PURE */\nconsole.log('b');\n`,
        });
        const makePlugins = (calls: string[], seen: Seen): Plugin[] => {
          let bDone!: () => void;
          const bIsDone = new Promise<void>((resolve) => (bDone = resolve));
          const resolveHook = {
            cache: true,
            async handler(this: PluginContext, source: string, importer: string | undefined) {
              if (source !== './b.js' || !importer?.endsWith('a.js')) return null;
              calls.push('resolve:a.js');
              // After the edit, only `a.js` imports `b.js`, so the hooks of `b.js` run later.
              await Promise.race([bIsDone, sleep(200)]);
              await sleep(20);
              const resolved = (await this.resolve(source, importer, { skipSelf: true }))!;
              switch (name) {
                case 'an object without `moduleSideEffects`':
                  return { id: resolved.id };
                case 'a spread of a `this.resolve` result':
                  return { ...resolved };
                case 'its own object with the read value':
                  return { id: resolved.id, moduleSideEffects: resolved.moduleSideEffects };
                case 'a `this.resolve` result with a new value':
                  resolved.moduleSideEffects = true;
                  return resolved;
                default:
                  return resolved;
              }
            },
          };
          return [
            {
              name: 'pure',
              transform: {
                cache: true,
                filter: { id: /[\\/]b\.js$/ },
                handler(code) {
                  calls.push('transform:b.js');
                  bDone();
                  return code.includes('PURE') ? { code, moduleSideEffects: false } : null;
                },
              },
            },
            {
              name: 'pass-on',
              ...(dynamic ? { resolveDynamicImport: resolveHook } : { resolveId: resolveHook }),
            },
            readerPlugin(seen),
          ];
        };
        const logs: RolldownLog[] = [];
        await build({ plugins: makePlugins([], {}), onLog: (_level, log) => void logs.push(log) });
        const hook = dynamic ? 'resolveDynamicImport' : 'resolveId';
        const warnings = moduleInfoWarnings(logs);
        if (use) {
          const [warning, ...more] = warnings;
          expect(more).toEqual([]);
          expect(warning).toMatchObject({ plugin: 'pass-on', hook, id: path.join(src, 'a.js') });
          expect(warning.message).toContain(
            {
              meta: 'read or changed the `meta` of "' + path.join(src, 'b.js') + '"',
              read: 'read the `moduleSideEffects` of "' + path.join(src, 'b.js') + '"',
            }[use],
          );
          expect(warning.message).toContain(
            'Rolldown prints this warning once for each plugin and hook, so other modules can have the same problem.',
          );
        } else {
          expect(warnings).toEqual([]);
        }

        // Now only `a.js` gives the resolve result that makes `b.js`, and `b.js` has side effects.
        writeFiles({
          'main.js': `import './a.js';\nconsole.log('main');\n`,
          'b.js': `console.log('b');\n`,
        });
        const cleanSeen: Seen = {};
        const cleanOutput = await build({ plugins: makePlugins([], cleanSeen), cache: undefined });
        const code = cleanOutput.map((file) => (file.type === 'chunk' ? file.code : '')).join('\n');
        expect(code).toContain(`console.log("b")`);

        const calls: string[] = [];
        const seen: Seen = {};
        const output = await build({ plugins: makePlugins(calls, seen) });
        // A saved result of `a.js` would make `b.js` a module without side effects.
        expect(calls.sort()).toEqual(
          reused ? ['transform:b.js'] : ['resolve:a.js', 'transform:b.js'],
        );
        expect(seenAfterScan(seen)).toEqual(seenAfterScan(cleanSeen));
        expect(output).toEqual(cleanOutput);
      });

      test('a `transform` hook passes on a value that a resolve hook of another module returned', async () => {
        // `main.js` asks a resolve hook for `t.js`, and that hook returns `false`. `b.js` then
        // imports `t.js` through the resolver of Rolldown, without a JavaScript resolve result.
        writeFiles({
          'main.js': `// ASK\nimport './b.js';\nconsole.log('main');\n`,
          'b.js': `import './t.js';\nconsole.log('b');\n`,
          't.js': `console.log('t');\n`,
        });
        const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
          {
            name: 'pure',
            resolveId: {
              cache: true,
              handler(source, importer, options) {
                if (source !== './t.js' || !options.custom?.ask) return null;
                calls.push(`resolveId:${rel(importer!)}`);
                return { id: path.join(src, 't.js'), moduleSideEffects: false };
              },
            },
          },
          {
            name: 'object-result',
            transform: {
              cache: true,
              async handler(code, id) {
                if (!id.startsWith(src)) return null;
                calls.push(`transform:${rel(id)}`);
                if (code.includes('ASK')) {
                  await this.resolve('./t.js', id, { custom: { ask: true } });
                }
                return rel(id) === 't.js' ? { code } : null;
              },
            },
          },
          readerPlugin(seen),
        ]);

        const code = clean.output
          .map((file) => (file.type === 'chunk' ? file.code : ''))
          .join('\n');
        expect(code).not.toContain(`console.log("t")`);
        // A saved result of `t.js` with no value would keep `t.js`.
        expect(restart.calls).toEqual(['transform:t.js']);
        for (const run of [clean, first, restart]) {
          expect(seenAfterScan(run.seen)).toEqual(seenAfterScan(clean.seen));
          expect(run.output).toEqual(clean.output);
          expect(moduleInfoWarnings(run.logs)).toEqual([]);
        }
      });

      test('a `transform` hook reads the value of another module through `this.getModuleInfo`', async () => {
        // `main.js` imports `b.js` first, so the hooks of `b.js` can end before `a.js` reads it.
        writeFiles({
          'main.js': `import { b } from './b.js';\nimport { a } from './a.js';\nconsole.log(a, b);\n`,
          'a.js': `export const a = { sideEffects: __B_SIDE_EFFECTS__ };\n`,
          'b.js': `/* PURE */\nexport const b = 'b';\n`,
        });
        const makePlugins = (calls: string[]): Plugin[] => {
          let bDone!: () => void;
          const bIsDone = new Promise<void>((resolve) => (bDone = resolve));
          return [
            {
              name: 'pure',
              transform: {
                cache: true,
                filter: { id: /[\\/]b\.js$/ },
                handler(code) {
                  calls.push('transform:b.js');
                  bDone();
                  return code.includes('PURE') ? { code, moduleSideEffects: false } : null;
                },
              },
            },
            {
              name: 'side-effects-reader',
              transform: {
                cache: true,
                filter: { id: /[\\/]a\.js$/ },
                async handler(code) {
                  calls.push('transform:a.js');
                  await Promise.race([bIsDone, sleep(200)]);
                  await sleep(20);
                  const info = await waitForModuleInfo(this, path.join(src, 'b.js'));
                  return code.replace('__B_SIDE_EFFECTS__', JSON.stringify(info.moduleSideEffects));
                },
              },
            },
          ];
        };
        const logs: RolldownLog[] = [];
        const before = await build({
          plugins: makePlugins([]),
          onLog: (_level, log) => void logs.push(log),
        });
        const chunkCode = (output: typeof before) =>
          output.map((file) => (file.type === 'chunk' ? file.code : '')).join('\n');
        expect(chunkCode(before)).toContain('sideEffects: false');
        const [warning, ...more] = moduleInfoWarnings(logs);
        expect(more).toEqual([]);
        expect(warning).toMatchObject({
          plugin: 'side-effects-reader',
          hook: 'transform',
          id: path.join(src, 'a.js'),
        });
        expect(warning.message).toContain(
          'read the `moduleSideEffects` of "' + path.join(src, 'b.js') + '"',
        );

        writeFiles({ 'b.js': `export const b = 'b';\n` });
        const cleanOutput = await build({ plugins: makePlugins([]), cache: undefined });
        expect(chunkCode(cleanOutput)).toContain('sideEffects: null');

        const calls: string[] = [];
        const restartLogs: RolldownLog[] = [];
        const output = await build({
          plugins: makePlugins(calls),
          onLog: (_level, log) => void restartLogs.push(log),
        });
        // A saved result of `a.js` would keep the old value `false`.
        expect(calls.sort()).toEqual(['transform:a.js', 'transform:b.js']);
        expect(output).toEqual(cleanOutput);
        expect(moduleInfoWarnings(restartLogs)).toEqual([
          expect.objectContaining({ plugin: 'side-effects-reader', hook: 'transform' }),
        ]);
      });

      test('a resolve hook reads the value of a `this.resolve` result before any hook set it', async () => {
        writeFiles({
          'main.js': `import v from './a.js';\nimport './b.js';\nconsole.log(v);\n`,
          'a.js': `import v from 'virtual';\nexport default v;\n`,
          'b.js': `console.log('b');\n`,
        });
        const makePlugins = (calls: string[], pureB: boolean): Plugin[] => [
          {
            name: 'pure',
            resolveId: {
              cache: true,
              handler(source, importer) {
                // The edit of `main.js` below makes its module task run again, so `pureB` takes
                // effect.
                if (!pureB || source !== './b.js' || rel(importer ?? '') !== 'main.js') return null;
                return { id: path.join(src, 'b.js'), moduleSideEffects: false };
              },
            },
          },
          {
            name: 'side-effects-reader',
            resolveId: {
              cache: true,
              async handler(source, importer) {
                if (source !== 'virtual') return null;
                calls.push(`resolveId:${rel(importer!)}`);
                const resolved = (await this.resolve('./b.js', importer))!;
                return `\0virtual:${resolved.moduleSideEffects}`;
              },
            },
            load: {
              cache: true,
              handler(id) {
                if (!id.startsWith('\0virtual:')) return null;
                return `export default ${JSON.stringify(id.slice('\0virtual:'.length))};`;
              },
            },
          },
        ];
        const logs: RolldownLog[] = [];
        await build({
          plugins: makePlugins([], false),
          onLog: (_level, log) => void logs.push(log),
        });
        const [warning, ...more] = moduleInfoWarnings(logs);
        expect(more).toEqual([]);
        expect(warning).toMatchObject({
          plugin: 'side-effects-reader',
          hook: 'resolveId',
          id: path.join(src, 'a.js'),
        });
        expect(warning.message).toContain(
          'read the `moduleSideEffects` of "' + path.join(src, 'b.js') + '"',
        );

        // Now `main.js` resolves `b.js` first, and the `pure` hook sets a value for it.
        writeFiles({ 'main.js': `import './b.js';\nimport v from './a.js';\nconsole.log(v);\n` });
        const cleanOutput = await build({ plugins: makePlugins([], true), cache: undefined });
        const code = cleanOutput.map((file) => (file.type === 'chunk' ? file.code : '')).join('\n');
        expect(code).toContain('"false"');

        const calls: string[] = [];
        const output = await build({ plugins: makePlugins(calls, true) });
        // A saved result of `a.js` would keep the old value `null`.
        expect(calls).toEqual(['resolveId:a.js']);
        expect(output).toEqual(cleanOutput);
      });
    });

    test('a read of `moduleSideEffects` of the module itself gives no saved result', async () => {
      writeFiles(BASIC_FILES);
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        {
          name: 'side-effects-reader',
          transform: {
            cache: true,
            handler(code, id) {
              if (!id.startsWith(src)) return null;
              calls.push(rel(id));
              if (rel(id) === 'a.js' && this.getModuleInfo(id)!.moduleSideEffects !== null) {
                throw new Error('a.js has a `moduleSideEffects` value');
              }
              return code.replaceAll('__NAME__', JSON.stringify(rel(id)));
            },
          },
        },
        readerPlugin(seen),
      ]);
      expect(restart.calls).toEqual(['a.js']);
      expect(moduleInfoWarnings(clean.logs)).toEqual([]);
      for (const run of [first, restart]) {
        expect(run.seen).toEqual(clean.seen);
        expect(run.output).toEqual(clean.output);
        const [warning, ...more] = moduleInfoWarnings(run.logs);
        expect(more).toEqual([]);
        expect(warning).toMatchObject({
          plugin: 'side-effects-reader',
          hook: 'transform',
          id: path.join(src, 'a.js'),
        });
        expect(warning.message).toContain(
          'read the `moduleSideEffects` of "' + path.join(src, 'a.js') + '"',
        );
      }
    });

    test('a use of `meta` or `moduleSideEffects` after the hook call ended does not count, and is not saved', async () => {
      writeFiles(BASIC_FILES);
      const makePlugins = (calls: string[], seen: Seen): Plugin[] => {
        let keptInfo: ReturnType<PluginContext['getModuleInfo']> | undefined;
        let keptGetModuleInfo: PluginContext['getModuleInfo'] | undefined;
        return [
          {
            name: 'keeper',
            transform: {
              cache: true,
              handler(code, id) {
                if (!id.startsWith(src)) return null;
                calls.push(rel(id));
                if (rel(id) === 'a.js') {
                  // Not a use of `meta`.
                  keptGetModuleInfo = this.getModuleInfo;
                  keptInfo = this.getModuleInfo(id)!;
                }
                return code.replaceAll('__NAME__', JSON.stringify(rel(id)));
              },
            },
            buildEnd() {
              if (!keptInfo || !keptGetModuleInfo) return;
              // `transform` ended long ago. Also a value that cannot be saved.
              keptInfo.meta.icons = ['home'];
              keptInfo.meta.late = new Set(['late']);
              void keptInfo.moduleSideEffects;
              keptInfo.moduleSideEffects = 'no-treeshake';
              keptGetModuleInfo(keptInfo.id)!.meta.fromKeptContext = true;
            },
          },
          readerPlugin(seen),
        ];
      };
      const { clean, first, restart } = await buildThreeTimes(makePlugins);
      const again = await build({ plugins: makePlugins([], {}) });

      // While the hook runs, the cache changes nothing.
      const changed = {
        meta: { icons: ['home'], late: new Set(['late']), fromKeptContext: true },
        moduleSideEffects: 'no-treeshake',
      };
      expect(clean.seen['generateBundle:a.js']).toEqual(changed);
      expect(first.seen).toEqual(clean.seen);
      expect(first.output).toEqual(clean.output);
      expect(first.logs).toEqual(clean.logs);

      // After a restart, the hook does not run, so the late changes are not made.
      expect(restart.calls).toEqual([]);
      expect(restart.seen['generateBundle:a.js']).toEqual({ meta: {}, moduleSideEffects: null });
      expect(restart.logs).toEqual(clean.logs);
      expect(again).toEqual(restart.output);
    });

    test('a rebuild in watch mode gives the same `meta` as without the cache', async () => {
      const original = `/* key: first */\nexport const a = 'a';\n`;
      const files = {
        'main.js': `import { a } from './a.js';\nimport { b } from './b.js';\nconsole.log(a, b);\n`,
        'a.js': original,
        'b.js': `/* key: b */\nexport const b = 'b';\n`,
        'unsavable.js': `export {};\n`,
      };
      const edits = [`/* key: second */\nexport const a = 'a, longer';\n`, original];

      // Runs the first build and one rebuild for each edit of `a.js`, all in one watcher.
      const runWatch = async (cache: CacheOption) => {
        writeFiles(files);
        const calls: string[] = [];
        const warnings: RolldownLog[] = [];
        let seen: Seen = {};
        const watcher = watch({
          input: [path.join(src, 'main.js'), path.join(src, 'unsavable.js')],
          cwd: root,
          output: { format: 'esm' },
          experimental: { cache },
          // Polling compares whole seconds, so each edit below waits one second first.
          watch: { skipWrite: true, watcher: { usePolling: true, pollInterval: 50 } },
          onLog(_level, log) {
            if (log.code === 'PERSISTENT_CACHE_UNSAVABLE_META') warnings.push(log);
          },
          plugins: [
            {
              name: 'keys',
              transform: {
                cache: true,
                handler(code, id) {
                  if (!id.startsWith(src)) return null;
                  calls.push(rel(id));
                  if (rel(id) === 'unsavable.js') return { meta: { map: new Map([[1, 2]]) } };
                  const key = /key: (\w+)/.exec(code)?.[1];
                  // The key of the meta changes with the file, and `shared` is replaced.
                  return key ? { meta: { [key]: rel(id), shared: key } } : null;
                },
              },
            },
            {
              name: 'reader',
              buildEnd() {
                seen = {};
                for (const id of this.getModuleIds()) {
                  if (!id.startsWith(src)) continue;
                  const info = this.getModuleInfo(id)!;
                  seen[rel(id)] = structuredClone({
                    meta: info.meta,
                    moduleSideEffects: info.moduleSideEffects,
                  });
                }
              },
            },
          ],
        });
        let onEnd: { resolve: () => void; reject: (error: unknown) => void } | undefined;
        const nextEnd = () =>
          new Promise<void>((resolve, reject) => {
            onEnd = { resolve, reject };
          });
        watcher.on('event', async (event) => {
          if (event.code === 'ERROR') onEnd?.reject(event.error);
          if (event.code === 'BUNDLE_END') await event.result.close();
          if (event.code === 'END') onEnd?.resolve();
        });
        const builds: { seen: Seen; calls: string[] }[] = [];
        try {
          let end = nextEnd();
          for (const edit of [undefined, ...edits]) {
            if (edit !== undefined) {
              end = nextEnd();
              await sleep(1000);
              writeFiles({ 'a.js': edit });
            }
            await end;
            builds.push({ seen, calls: calls.splice(0) });
          }
        } finally {
          await watcher.close();
        }
        return { builds, warnings };
      };

      const clean = await runWatch(undefined);
      const cached = await runWatch({ dir: cacheDir });

      const unsavable = { meta: { map: new Map([[1, 2]]) }, moduleSideEffects: null };
      const b = { meta: { b: 'b.js', shared: 'b' }, moduleSideEffects: null };
      const main = { meta: {}, moduleSideEffects: null };
      const metaOfA = [
        { first: 'a.js', shared: 'first' },
        { first: 'a.js', second: 'a.js', shared: 'second' },
        // The file is as in the first build. The key `second` stays from the second build.
        { first: 'a.js', second: 'a.js', shared: 'first' },
      ];
      expect(clean.builds.map((build) => build.seen)).toEqual(
        metaOfA.map((meta) => ({
          'main.js': main,
          'a.js': { meta, moduleSideEffects: null },
          'b.js': b,
          'unsavable.js': unsavable,
        })),
      );
      expect(cached.builds.map((build) => build.seen)).toEqual(
        clean.builds.map((build) => build.seen),
      );

      // Without the cache, a rebuild runs the hooks of every module.
      expect(clean.builds[1].calls).toContain('b.js');
      // With the cache, the rebuild reuses the saved result of `b.js`, and runs `a.js`.
      expect(cached.builds[1].calls).toContain('a.js');
      expect(cached.builds[1].calls).not.toContain('b.js');
      expect(cached.builds[2].calls).not.toContain('b.js');
      // `unsavable.js` has no saved result, so its hook runs in every build.
      for (const build of cached.builds) {
        expect(build.calls).toContain('unsavable.js');
      }
      // The warning is printed once for the whole life of the bundler, not once for each build.
      expect(clean.warnings).toEqual([]);
      expect(cached.warnings).toHaveLength(1);
      expect(cached.warnings[0].id).toBe(path.join(src, 'unsavable.js'));
    }, 60_000);

    /**
     * Runs a first build and one rebuild for each edit, all in one watcher. Returns what `read`
     * gives after each build, with the hook calls of that build.
     */
    async function watchBuilds<T>(options: {
      cache: CacheOption;
      edits: Record<string, string>[];
      makePlugins: (calls: string[]) => Plugin[];
      read: () => T;
      treeshake?: InputOptions['treeshake'];
    }): Promise<{ result: T; calls: string[] }[]> {
      const calls: string[] = [];
      let lastBuildStart = 0;
      const watcher = watch({
        input: path.join(src, 'main.js'),
        cwd: root,
        output: { format: 'esm', dir: path.join(root, 'dist') },
        treeshake: options.treeshake,
        experimental: { cache: options.cache },
        // Polling compares whole seconds, so each edit below waits one second first.
        watch: { watcher: { usePolling: true, pollInterval: 50 } },
        plugins: [
          {
            name: 'build-start',
            buildStart() {
              lastBuildStart = performance.now();
              calls.length = 0;
            },
          },
          ...options.makePlugins(calls),
        ],
      });
      let onEnd: { resolve: () => void; reject: (error: unknown) => void } | undefined;
      const nextEnd = () =>
        new Promise<void>((resolve, reject) => {
          onEnd = { resolve, reject };
        });
      watcher.on('event', async (event) => {
        if (event.code === 'ERROR') onEnd?.reject(event.error);
        if (event.code === 'BUNDLE_END') await event.result.close();
        if (event.code === 'END') onEnd?.resolve();
      });
      const builds: { result: T; calls: string[] }[] = [];
      try {
        for (const edit of [undefined, ...options.edits]) {
          let written = 0;
          if (edit !== undefined) {
            await sleep(1000);
            writeFiles(edit);
            written = performance.now();
          }
          // The watcher can build two times for one edit. Only a build that started after the
          // edit is sure to have the new file.
          do {
            await nextEnd();
          } while (lastBuildStart < written);
          builds.push({ result: options.read(), calls: [...calls] });
        }
      } finally {
        await watcher.close();
      }
      return builds;
    }

    const outputHas = (text: string) =>
      fs.readFileSync(path.join(root, 'dist', 'main.js'), 'utf8').includes(text);

    test('watch mode: a saved result with a returned `moduleSideEffects` replaces an older assignment', async () => {
      // main.js -> b.js -> c.js. So the hooks of `c.js` run after the hooks of `b.js`.
      const cPlain = `console.log('c');\n`;
      const cAssigns = `/* ASSIGN */\nconsole.log('c');\n`;
      const files = {
        'main.js': `import './b.js';\nconsole.log('main');\n`,
        'b.js': `import './c.js';\nconsole.log('side effect of b');\n`,
        'c.js': cPlain,
      };
      const run = (cache: CacheOption) => {
        writeFiles(files);
        return watchBuilds({
          cache,
          // The third build has the same files as the first one.
          edits: [{ 'c.js': cAssigns }, { 'c.js': cPlain }],
          read: () => outputHas('side effect of b'),
          makePlugins: () => [
            {
              name: 'side-effects',
              transform: {
                cache: true,
                handler(code, id) {
                  if (rel(id) === 'b.js') return { code, moduleSideEffects: false };
                  if (rel(id) === 'c.js' && code.includes('ASSIGN')) {
                    this.getModuleInfo(path.join(src, 'b.js'))!.moduleSideEffects = true;
                  }
                  return null;
                },
              },
            },
          ],
        });
      };

      const clean = (await run(undefined)).map((build) => build.result);
      // `b.js` is kept only while the hook of `c.js` assigns `true`.
      expect(clean).toEqual([false, true, false]);
      // In the third build, `b.js` and `c.js` reuse their saved results. The JavaScript side
      // still holds the assignment from the second build. The returned `false` in the saved
      // result of `b.js` is newer.
      const cached = (await run({ dir: cacheDir })).map((build) => build.result);
      expect(cached).toEqual(clean);
    }, 60_000);

    test('watch mode: hooks that use `meta` through `this.getModuleInfo` run in every build', async () => {
      const writer = (text: string) => `/* ${text} */\nconsole.log('writer');\n`;
      const files = {
        'main.js': `import './writer.js';\nimport './assigner.js';\nimport './b.js';\nconsole.log('main');\n`,
        'writer.js': writer('first'),
        'assigner.js': `console.log('assigner');\n`,
        'b.js': `console.log('side effect of b');\n`,
      };
      type Result = { seen: Seen; hasB: boolean };
      const run = (cache: CacheOption, edits: Record<string, string>[]) => {
        let seen: Seen = {};
        return watchBuilds<Result>({
          cache,
          edits,
          treeshake: { moduleSideEffects: false },
          read: () => ({ seen, hasB: outputHas('side effect of b') }),
          makePlugins: (calls) => [
            {
              name: 'writes',
              transform: {
                cache: true,
                async handler(code, id) {
                  if (!id.startsWith(src)) return null;
                  calls.push(rel(id));
                  if (rel(id) === 'writer.js') {
                    const meta = this.getModuleInfo(id)!.meta;
                    // The same value in every build, and a value that changes with the file.
                    meta.flag = true;
                    meta.text = /\/\* (\w+)/.exec(code)![1];
                  }
                  if (rel(id) === 'assigner.js') {
                    this.getModuleInfo(id)!.meta.own = ['assigner'];
                    const b = await waitForModuleInfo(this, path.join(src, 'b.js'));
                    b.meta.fromAssigner = true;
                    b.moduleSideEffects = true;
                  }
                  return null;
                },
              },
            },
            {
              name: 'reader',
              buildEnd() {
                seen = {};
                for (const id of this.getModuleIds()) {
                  if (!id.startsWith(src)) continue;
                  const info = this.getModuleInfo(id)!;
                  seen[rel(id)] = structuredClone({
                    meta: info.meta,
                    moduleSideEffects: info.moduleSideEffects,
                  });
                }
              },
            },
          ],
        });
      };
      const edits = [{ 'writer.js': writer('second') }];

      writeFiles(files);
      const clean = await run(undefined, edits);
      // The watcher starts again with the files of the last build.
      const cleanAgain = await run(undefined, []);
      expect(clean.map((build) => build.result)).toMatchObject([
        { seen: { 'writer.js': { meta: { flag: true, text: 'first' } } }, hasB: true },
        {
          seen: {
            'writer.js': { meta: { flag: true, text: 'second' } },
            'assigner.js': { meta: { own: ['assigner'] } },
            'b.js': { meta: { fromAssigner: true }, moduleSideEffects: true },
          },
          hasB: true,
        },
      ]);
      expect(cleanAgain[0].result).toEqual(clean[1].result);

      writeFiles(files);
      const cached = await run({ dir: cacheDir }, edits);
      const cachedAgain = await run({ dir: cacheDir }, []);
      expect(cached.map((build) => build.result)).toEqual(clean.map((build) => build.result));
      expect(cachedAgain[0].result).toEqual(cleanAgain[0].result);

      // `main.js` and `b.js` reuse their saved results.
      expect(cached[1].calls.sort()).toEqual(['assigner.js', 'writer.js']);
      expect(cachedAgain[0].calls.sort()).toEqual(['assigner.js', 'writer.js']);
    }, 60_000);
  });

  describe('hooks that are not cached', () => {
    test('`this.markNotCacheable` gives the module of the hook call no saved result', async () => {
      writeFiles(BASIC_FILES);
      const makePlugins = (calls: string[]): Plugin[] => [
        recordingPlugin(calls, true),
        {
          name: 'marker',
          load: {
            cache: true,
            handler(id) {
              if (id === path.join(src, 'b.js')) this.markNotCacheable('test: load of b.js');
              return null;
            },
          },
          transform: {
            cache: true,
            handler(_code, id) {
              if (id === path.join(src, 'a.js')) this.markNotCacheable();
              return null;
            },
          },
          buildEnd() {
            // Not a hook call of a module task: it does nothing.
            this.markNotCacheable();
          },
        },
      ];
      const out1 = await build({ plugins: makePlugins([]) });
      for (let i = 0; i < 2; i++) {
        const calls: string[] = [];
        const out = await build({ plugins: makePlugins(calls) });
        expect(calls.sort()).toEqual([
          'recording.load:a.js',
          'recording.load:b.js',
          'recording.resolveId:a.js:./c.js',
          'recording.transform:a.js',
          'recording.transform:b.js',
        ]);
        expect(out).toEqual(out1);
      }
      // Without the cache, the call does nothing.
      expect(out1).toEqual(await cleanBuild(() => makePlugins([])));
    });

    describe.skipIf(isWasiTest)('parallel plugin', () => {
      const parallelCachedPlugin = defineParallelPlugin<{
        src: string;
        log: string;
        cache?: boolean;
        only?: string;
      }>(path.join(import.meta.dirname, 'parallel-cached-plugin-impl.js'));

      /** Builds two times. Returns the modules that the hook of the parallel plugin ran for. */
      async function buildTwice(
        cache: CacheOption,
        pluginOptions: { cache?: boolean; only?: string },
      ) {
        const log = path.join(root, 'parallel.log');
        const run = async () => {
          fs.writeFileSync(log, '');
          const calls: string[] = [];
          const output = await build({
            plugins: [
              recordingPlugin(calls, true),
              parallelCachedPlugin({ src, log, ...pluginOptions }) as unknown as Plugin,
            ],
            cache,
          });
          const parallel = fs.readFileSync(log, 'utf8').split('\n').filter(Boolean).map(rel);
          return { output, calls: calls.sort(), parallel: parallel.sort() };
        };
        return { first: await run(), second: await run() };
      }

      test('a hook with `cache: true` runs again for every module', async () => {
        writeFiles(BASIC_FILES);
        const { first, second } = await buildTwice({ dir: cacheDir }, { cache: true });
        expect(first.parallel).toEqual(['a.js', 'b.js', 'c.js', 'main.js']);
        expect(second.parallel).toEqual(first.parallel);
        // No module has a saved result, so the cached hooks of the other plugin run again too.
        expect(second.calls).toEqual(first.calls);
        expect(second.output).toEqual(first.output);
      });

      test('`assumeCacheableHooks` does not turn the cache on for its hooks', async () => {
        writeFiles(BASIC_FILES);
        const { first, second } = await buildTwice(
          {
            dir: cacheDir,
            assumeCacheableHooks: { resolveId: true, load: true, transform: true },
          },
          {},
        );
        expect(second.parallel).toEqual(['a.js', 'b.js', 'c.js', 'main.js']);
        expect(second.calls).toEqual(first.calls);
      });

      test('a module that its hook filter blocks still gets a saved result', async () => {
        writeFiles(BASIC_FILES);
        const { first, second } = await buildTwice(
          { dir: cacheDir },
          { cache: true, only: '[\\\\/]a\\.js$' },
        );
        expect(first.parallel).toEqual(['a.js']);
        expect(second.parallel).toEqual(['a.js']);
        expect(second.calls).toEqual([
          'recording.load:a.js',
          'recording.resolveId:a.js:./c.js',
          'recording.transform:a.js',
        ]);
        expect(second.output).toEqual(first.output);
      });
    });
  });

  test('`this.warn` in a cached `transform` reaches `onLog` again', async () => {
    writeFiles(BASIC_FILES);
    const makePlugins = (calls: string[]): Plugin[] => [
      recordingPlugin(calls, true),
      {
        name: 'warner',
        transform: {
          cache: true,
          filter: { id: /[\\/]b\.js$/ },
          handler() {
            this.warn({ message: 'look at b', code: 'MY_CODE', meta: { n: 1 } }, 7);
            return null;
          },
        },
      },
    ];
    const collect = () => {
      const logs: { level: string; log: RolldownLog }[] = [];
      const onLog: InputOptions['onLog'] = (level, log) => {
        if (log.plugin === 'warner') logs.push({ level, log });
      };
      return { logs, onLog };
    };

    const first = collect();
    await build({ plugins: makePlugins([]), onLog: first.onLog });
    expect(first.logs).toHaveLength(1);
    expect(first.logs[0].log).toMatchObject({
      message: expect.stringContaining('look at b'),
      code: 'PLUGIN_WARNING',
      pluginCode: 'MY_CODE',
      plugin: 'warner',
      hook: 'transform',
      id: path.join(src, 'b.js'),
      pos: 7,
    });

    const second = collect();
    const calls: string[] = [];
    await build({ plugins: makePlugins(calls), onLog: second.onLog });
    expect(calls).toEqual([]);
    expect(second.logs).toEqual([
      { level: first.logs[0].level, log: JSON.parse(JSON.stringify(first.logs[0].log)) },
    ]);
  });

  test('`this.emitFile` in a cached `transform` emits the asset again', async () => {
    writeFiles(BASIC_FILES);
    const makePlugins = (calls: string[]): Plugin[] => [
      recordingPlugin(calls, true),
      {
        name: 'emitter',
        transform: {
          cache: true,
          filter: { id: /[\\/]b\.js$/ },
          handler(code) {
            const ref = this.emitFile({ type: 'asset', name: 'data.txt', source: 'hello' });
            return code + `\nexport const url = import.meta.ROLLUP_FILE_URL_${ref};\n`;
          },
        },
      },
    ];
    const out1 = await build({ plugins: makePlugins([]) });
    expect(out1.find((file) => file.type === 'asset')).toMatchObject({
      names: ['data.txt'],
      source: 'hello',
    });

    const calls: string[] = [];
    const out2 = await build({ plugins: makePlugins(calls) });
    expect(calls).toEqual([]);
    expect(out2).toEqual(out1);
  });

  test('nothing is written when the cache is off', async () => {
    writeFiles(BASIC_FILES);
    for (const cache of [false, undefined] as const) {
      const calls: string[] = [];
      await build({ plugins: [recordingPlugin([], true)], cache });
      await build({ plugins: [recordingPlugin(calls, true)], cache });
      expect(calls.filter((c) => c.includes('.transform:')).length).toBe(4);
    }
    expect(fs.readdirSync(root)).toEqual(['src']);
  });

  test('changing `cacheKey` runs every hook again', async () => {
    writeFiles(BASIC_FILES);
    await build({ plugins: [recordingPlugin([], true)], cache: { dir: cacheDir, cacheKey: 'a' } });
    const same: string[] = [];
    await build({
      plugins: [recordingPlugin(same, true)],
      cache: { dir: cacheDir, cacheKey: 'a' },
    });
    expect(same).toEqual([]);
    const changed: string[] = [];
    await build({
      plugins: [recordingPlugin(changed, true)],
      cache: { dir: cacheDir, cacheKey: 'b' },
    });
    expect(changed.filter((c) => c.includes('.transform:')).length).toBe(4);
  });

  test('changing a build dependency runs every hook again', async () => {
    writeFiles(BASIC_FILES);
    const config = path.join(root, 'config.js');
    fs.writeFileSync(config, 'export default 1;\n');
    const cache: CacheOption = { dir: cacheDir, buildDependencies: [config] };
    await build({ plugins: [recordingPlugin([], true)], cache });
    const same: string[] = [];
    await build({ plugins: [recordingPlugin(same, true)], cache });
    expect(same).toEqual([]);

    fs.writeFileSync(config, 'export default 22;\n');
    const changed: string[] = [];
    await build({ plugins: [recordingPlugin(changed, true)], cache });
    expect(changed.filter((c) => c.includes('.transform:')).length).toBe(4);
  });

  test('results saved in watch mode are not reused by a build outside watch mode', async () => {
    writeFiles(BASIC_FILES);
    // Both builds get the same options, so only `this.meta.watchMode` differs. Plugins can read
    // it, so it is an input of the saved results.
    const options = (calls: string[]): InputOptions => ({
      input: path.join(src, 'main.js'),
      cwd: root,
      plugins: [recordingPlugin(calls, true)],
      experimental: { cache: { dir: cacheDir } },
      watch: { skipWrite: true },
    });
    const runWatch = async (calls: string[]) => {
      const watcher = watch({ ...options(calls), output: { format: 'esm' } });
      await new Promise<void>((resolve, reject) => {
        watcher.on('event', async (event) => {
          if (event.code === 'ERROR') reject(event.error);
          if (event.code === 'BUNDLE_END') await event.result.close();
          if (event.code === 'END') resolve();
        });
      });
      await watcher.close();
    };
    const runBuild = async (calls: string[]) => {
      const bundle = await rolldown(options(calls));
      await bundle.generate({ format: 'esm' });
      await bundle.close();
    };

    await runWatch([]);
    const watchAgain: string[] = [];
    await runWatch(watchAgain);
    expect(watchAgain).toEqual([]);

    const outside: string[] = [];
    await runBuild(outside);
    expect(outside.filter((c) => c.includes('.transform:')).length).toBe(4);
    const buildAgain: string[] = [];
    await runBuild(buildAgain);
    expect(buildAgain).toEqual([]);
  });

  test('damaged cache files do not break the build', async () => {
    writeFiles(BASIC_FILES);
    const out1 = await build({ plugins: [recordingPlugin([], true)] });
    const files = listFiles(cacheDir);
    expect(files.length).toBeGreaterThan(0);
    for (const file of files) {
      const size = fs.statSync(file).size;
      fs.writeFileSync(file, Buffer.alloc(Math.max(size, 16), 0xa5));
    }
    const out2 = await build({ plugins: [recordingPlugin([], true)] });
    expect(out2).toEqual(out1);
  });
  test('`experimental.cache: true` is rejected', async () => {
    writeFiles(BASIC_FILES);
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    try {
      await expect(
        build({
          plugins: [],
          // @ts-expect-error `true` is not a valid value
          cache: true,
        }),
      ).rejects.toThrow(/`experimental\.cache`.*Rolldown has no default cache directory/);
      expect(warn).toHaveBeenCalledWith(expect.stringContaining('experimental.cache'));
    } finally {
      warn.mockRestore();
    }
  });
});
