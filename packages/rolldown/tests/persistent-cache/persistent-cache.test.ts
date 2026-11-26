import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  type InputOptions,
  type OutputOptions,
  type Plugin,
  type PluginContext,
  type RolldownLog,
  type RolldownOutput,
  rolldown,
  watch,
} from 'rolldown';
import { dev } from 'rolldown/experimental';
import { sleep } from 'rolldown-tests/utils';
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

interface BuildOptions {
  plugins: Plugin[];
  /** Defaults to `{ dir: cacheDir }`. Pass `undefined` explicitly to leave it unset. */
  cache?: CacheOption;
  onLog?: InputOptions['onLog'];
  output?: OutputOptions;
}

async function build(options: BuildOptions) {
  const cache = 'cache' in options ? options.cache : { dir: cacheDir };
  const bundle = await rolldown({
    input: path.join(src, 'main.js'),
    cwd: root,
    plugins: options.plugins,
    onLog: options.onLog,
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
            if (id.startsWith(src)) calls.push(`load:${rel(id)}`);
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

  describe('module meta and `moduleSideEffects`', () => {
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

    /** Builds without the cache, then two times with it. The last build is the restart. */
    async function buildThreeTimes<S extends object = Seen>(
      makePlugins: (calls: string[], seen: S) => Plugin[],
    ) {
      const run = async (cache: CacheOption) => {
        const calls: string[] = [];
        const seen = {} as S;
        const logs: RolldownLog[] = [];
        const output = await build({
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

    const unsavableMetaWarnings = (logs: RolldownLog[]) =>
      logs.filter((log) => log.code === 'PERSISTENT_CACHE_UNSAVABLE_META');

    test('the `meta` that cached `load` and `transform` hooks return is read again after a restart', async () => {
      writeFiles(BASIC_FILES);
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        recordingPlugin(calls, true),
        {
          name: 'icons',
          load: {
            cache: true,
            filter: { id: /[\\/]c\.js$/ },
            handler(id) {
              calls.push(`icons.load:${rel(id)}`);
              return { code: fs.readFileSync(id, 'utf8'), meta: { loaded: true, shared: 'load' } };
            },
          },
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
        },
        moduleSideEffects: true,
      });
      expect(clean.seen['renderChunk:c.js']?.meta).toEqual({
        loaded: true,
        shared: 'icons',
        icons: ['c.js', { nested: [1, 'two', null, true, { deep: {} }] }],
      });
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

    test('the `meta` of the resolution that created a module is replayed after a restart', async () => {
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
        moduleSideEffects: true,
      });
      expect(clean.seen['moduleParsed:c.js']).toEqual(clean.seen['renderChunk:c.js']);
      expect(clean.seen['renderChunk:a.js']).toEqual({ meta: {}, moduleSideEffects: true });

      expect(restart.calls).toEqual([]);
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
      expect(restart.output).toEqual(clean.output);
    });

    test('the `meta` of a second importer is not applied after a restart', async () => {
      // `main.js` creates `dep.js`: `late.js` is created from the imports of `main.js` too, so
      // its own import of `./dep.js` is resolved later.
      writeFiles({
        'main.js': `import './dep.js';\nimport './late.js';\nconsole.log('main');\n`,
        'late.js': `import './dep.js';\nconsole.log('late');\n`,
        'dep.js': `console.log('dep');\n`,
      });
      const { clean, first, restart } = await buildThreeTimes((calls, seen) => [
        {
          name: 'from-importer',
          resolveId: {
            cache: true,
            handler(source, importer) {
              if (source !== './dep.js' || !importer) return null;
              calls.push(`resolveId:${rel(importer)}`);
              return { id: path.join(src, 'dep.js'), meta: { from: rel(importer) } };
            },
          },
        },
        readerPlugin(seen),
      ]);

      expect(clean.calls.sort()).toEqual(['resolveId:late.js', 'resolveId:main.js']);
      expect(clean.seen['generateBundle:dep.js']?.meta).toEqual({ from: 'main.js' });
      expect(restart.calls).toEqual([]);
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
    });

    test('the `meta` of a saved resolution that creates an external module is replayed after a restart', async () => {
      writeFiles({ 'main.js': `import 'ext';\nconsole.log('main');\n` });
      const { clean, first, restart } = await buildThreeTimes<Record<string, unknown>>(
        (calls, seen) => [
          {
            name: 'external-meta',
            resolveId: {
              cache: true,
              handler(source, importer) {
                if (source !== 'ext' || !importer) return null;
                calls.push('resolveId');
                return { id: 'ext', external: true, meta: { from: rel(importer) } };
              },
            },
            generateBundle() {
              seen.ext = this.getModuleInfo('ext')?.meta;
            },
          },
        ],
      );

      expect(clean.seen).toEqual({ ext: { from: 'main.js' } });
      expect(restart.calls).toEqual([]);
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
    });

    test('a changed module merges its hook results into the saved `meta` of its creating resolution', async () => {
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
        readerPlugin(seen),
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
      expect(cleanSeen['generateBundle:c.js']?.meta).toEqual({
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

    /** The part of `seen` from the hooks that run after all module tasks. */
    const seenAfterScan = (seen: Seen) =>
      Object.fromEntries(
        Object.entries(seen).filter(
          ([key]) => key.startsWith('renderChunk:') || key.startsWith('generateBundle:'),
        ),
      );

    test('a nested `this.resolve` call that uses `meta` gives the outer module task no saved result', async () => {
      writeFiles(BASIC_FILES);
      const main = path.join(src, 'main.js');
      const { clean, restart } = await buildThreeTimes((calls, seen) => [
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
      // The hook changes `meta` through `this.getModuleInfo`, so its module task gets no saved
      // result. `main.js` keeps its saved result, which holds only its own `meta`.
      const collector = (
        calls: string[],
        push: (ctx: PluginContext, code: string, id: string) => void,
      ): Plugin => ({
        name: 'collector',
        transform: {
          cache: true,
          handler(code, id) {
            if (!id.startsWith(src)) return null;
            calls.push(rel(id));
            if (rel(id) === 'main.js') return { code, meta: { list: [] } };
            push(this, code, id);
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
        expect(calls).toEqual(['a.js', 'b.js']);
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
      writeFiles({
        'main.js': `import './a.js';\nconsole.log('main');\n`,
        'a.js': `/* FLAG */\nimport './b.js';\nconsole.log('a');\n`,
        'b.js': `/* FLAG */\nconsole.log('b');\n`,
      });
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

    test('the `meta` in a hook that the cache records is the object of the module', async () => {
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
      expect(restart.seen['generateBundle:a.js']).toEqual({ meta: {}, moduleSideEffects: true });
      expect(restart.logs).toEqual(clean.logs);
      expect(again).toEqual(restart.output);
    });

    /**
     * Runs a first build and one rebuild for each edit, all in one watcher. Returns what `read`
     * gives after each build, with the hook calls of that build.
     */
    async function watchBuilds<T>(options: {
      cache: CacheOption;
      edits: Record<string, string>[];
      makePlugins: (calls: string[]) => Plugin[];
      read: () => T;
      input?: string[];
      treeshake?: InputOptions['treeshake'];
      onLog?: InputOptions['onLog'];
    }): Promise<{ result: T; calls: string[] }[]> {
      const calls: string[] = [];
      let lastBuildStart = 0;
      const watcher = watch({
        input: options.input ?? path.join(src, 'main.js'),
        cwd: root,
        output: { format: 'esm', dir: path.join(root, 'dist') },
        treeshake: options.treeshake,
        onLog: options.onLog,
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

    test('watch mode: each rebuild gives the same `meta` as without the cache', async () => {
      const original = `/* key: first */\nexport const a = 'a';\n`;
      const files = {
        'main.js': `import { a } from './a.js';\nimport { b } from './b.js';\nconsole.log(a, b);\n`,
        'a.js': original,
        'b.js': `/* key: b */\nexport const b = 'b';\n`,
        'unsavable.js': `export {};\n`,
      };
      const run = (cache: CacheOption) => {
        writeFiles(files);
        let seen: Seen = {};
        const warnings: RolldownLog[] = [];
        return watchBuilds({
          cache,
          input: [path.join(src, 'main.js'), path.join(src, 'unsavable.js')],
          edits: [
            { 'a.js': `/* key: second */\nexport const a = 'a, longer';\n` },
            { 'a.js': original },
          ],
          onLog(_level, log) {
            if (log.code === 'PERSISTENT_CACHE_UNSAVABLE_META') warnings.push(log);
          },
          read: () => ({ seen, warnings: [...warnings] }),
          makePlugins: (calls) => [
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
      };

      const clean = await run(undefined);
      const cached = await run({ dir: cacheDir });

      const metaOfA = [
        { first: 'a.js', shared: 'first' },
        { second: 'a.js', shared: 'second' },
        // Each build starts fresh, so the key `second` of the second build is gone.
        { first: 'a.js', shared: 'first' },
      ];
      expect(clean.map((build) => build.result.seen)).toEqual(
        metaOfA.map((meta) => ({
          'main.js': { meta: {}, moduleSideEffects: true },
          'a.js': { meta, moduleSideEffects: true },
          'b.js': { meta: { b: 'b.js', shared: 'b' }, moduleSideEffects: true },
          'unsavable.js': { meta: { map: new Map([[1, 2]]) }, moduleSideEffects: true },
        })),
      );
      expect(cached.map((build) => build.result.seen)).toEqual(
        clean.map((build) => build.result.seen),
      );

      // Without the cache, a rebuild runs the hooks of every module.
      expect(clean[1].calls).toContain('b.js');
      // With the cache, the rebuild reuses the saved result of `b.js`, and runs `a.js`.
      expect(cached[1].calls).toContain('a.js');
      expect(cached[1].calls).not.toContain('b.js');
      expect(cached[2].calls).not.toContain('b.js');
      // `unsavable.js` has no saved result, so its hook runs in every build.
      for (const build of cached) {
        expect(build.calls).toContain('unsavable.js');
      }
      // The warning is printed once for the whole life of the bundler, not once for each build.
      expect(clean.at(-1)!.result.warnings).toEqual([]);
      expect(cached.at(-1)!.result.warnings).toHaveLength(1);
      expect(cached.at(-1)!.result.warnings[0].id).toBe(path.join(src, 'unsavable.js'));
    }, 60_000);

    test('watch mode: an assignment in an earlier build does not stay', async () => {
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
      // In the third build, `b.js` and `c.js` reuse their saved results.
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

    test('the `moduleSideEffects` that cached hooks return is read again after a restart', async () => {
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
          // A later hook that returns an object without `moduleSideEffects`. It keeps the value
          // that the module already has.
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
          'main.js': true,
          'plain.js': true,
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

      // The value of `resolve-false.js` is in the saved resolve results of `main.js`. The other
      // values are in the saved results of the modules themselves.
      expect(restart.calls).toEqual([]);
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
      expect(first.output).toEqual(clean.output);
      expect(restart.output).toEqual(clean.output);
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

      expect(clean.seen['renderChunk:a.js']?.meta).toEqual({ icons: new Set(['home', 'search']) });
      expect(clean.seen['renderChunk:b.js']?.meta).toEqual({ icons: ['home', 'search'] });
      expect(unsavableMetaWarnings(clean.logs)).toEqual([]);

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
        const warnings = unsavableMetaWarnings(logs);
        expect(warnings).toHaveLength(1);
        expect(warnings[0]).toMatchObject({ plugin: 'icons', hook: 'transform' });
        expect(warnings[0].message).toContain('"icons"');
        expect(warnings[0].message).toContain('"transform"');
        expect([path.join(src, 'a.js'), path.join(src, 'c.js')]).toContain(warnings[0].id);
        expect(warnings[0].message).toContain(warnings[0].id);
      }
    });

    test('a `meta` that is not a plain object, or that would change after a restart, gives no saved result', async () => {
      class Icons {
        list = ['home'];
        get count() {
          return this.list.length;
        }
      }
      const withoutPrototype = () => Object.assign(Object.create(null), { home: 1 });
      // For each module: the `meta` that its `load` hook returns, and what a later hook reads.
      // Only a plain object, or an object without a prototype, is supported as `meta`. A `Map`
      // has no own keys, so it adds nothing to the copy of the module, and there is nothing to
      // save.
      const cases: Record<string, { meta: () => any; read: (meta: any) => unknown }> = {
        'class.js': {
          meta: () => new Icons(),
          read: (meta) => [meta instanceof Icons, meta.count, meta.list],
        },
        'map.js': { meta: () => new Map([['icons', 1]]), read: (meta) => Object.keys(meta) },
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
        'symbol-key.js': {
          meta: () => ({ [Symbol.for('icons')]: 1, plain: 1 }),
          read: (meta) => Object.getOwnPropertySymbols(meta).length,
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
        'class.js': [false, undefined, ['home']],
        'map.js': [],
        'array.js': [false, 'home'],
        'no-prototype.js': [true, 1],
        'nested-no-prototype.js': [false, 1],
        'used-two-times.js': true,
        'symbol-key.js': 1,
        'half-pair-key.js': [0xd800],
        'plain.js': ['home'],
      });
      expect(restart.calls.sort()).toEqual([
        'array.js',
        'class.js',
        'half-pair-key.js',
        'nested-no-prototype.js',
        'symbol-key.js',
        'used-two-times.js',
      ]);
      expect(first.seen).toEqual(clean.seen);
      expect(restart.seen).toEqual(clean.seen);
      expect(restart.output).toEqual(clean.output);

      expect(unsavableMetaWarnings(clean.logs)).toEqual([]);
      for (const logs of [first.logs, restart.logs]) {
        expect(unsavableMetaWarnings(logs)).toHaveLength(1);
        expect(unsavableMetaWarnings(logs)[0]).toMatchObject({ plugin: 'shapes', hook: 'load' });
      }
    });

    test('a cached hook that uses `meta` or `moduleSideEffects` through `this.getModuleInfo` gets no saved result', async () => {
      const names = [
        'reads-meta.js',
        'writes-own-meta.js',
        'reads-side-effects.js',
        'assigns-side-effects.js',
        'other-info.js',
        'reads-resolve-meta.js',
        'reads-own-side-effects.js',
        'plain.js',
      ];
      writeFiles({
        'main.js': names.map((name) => `import './${name}';\n`).join('') + `console.log('main');\n`,
        ...Object.fromEntries(names.map((name) => [name, `console.log('${name}');\n`])),
      });
      const main = path.join(src, 'main.js');
      const { clean, first, restart } = await buildThreeTimes<Record<string, unknown>>(
        (calls, seen) => [
          {
            name: 'base',
            transform: {
              cache: true,
              filter: { id: /[\\/]main\.js$/ },
              handler() {
                return { meta: { base: 1 } };
              },
            },
          },
          {
            name: 'user',
            resolveId: {
              cache: true,
              handler(source, importer) {
                if (importer === main && source === './plain.js') {
                  // The task of `main.js` runs this hook, so `main.js` gets no saved result.
                  seen.resolveId = this.getModuleInfo(main)!.meta.base;
                }
                return source === 'virtual:with-meta'
                  ? { id: '\0with-meta', meta: { r: 1 } }
                  : null;
              },
            },
            transform: {
              cache: true,
              async handler(_code, id) {
                if (!id.startsWith(src)) return null;
                calls.push(rel(id));
                // `main.js` is parsed before its imports are transformed.
                const info = this.getModuleInfo(main)!;
                switch (rel(id)) {
                  case 'reads-meta.js':
                    seen.readsMeta = info.meta.base;
                    break;
                  case 'writes-own-meta.js':
                    this.getModuleInfo(id)!.meta.own = true;
                    break;
                  case 'reads-side-effects.js':
                    seen.readsSideEffects = info.moduleSideEffects;
                    break;
                  case 'assigns-side-effects.js':
                    this.getModuleInfo(id)!.moduleSideEffects = true;
                    break;
                  case 'other-info.js':
                    // Not a use of `meta` or `moduleSideEffects`.
                    seen.otherInfo = [info.isEntry, info.importedIds.length];
                    break;
                  case 'reads-resolve-meta.js': {
                    // A `this.resolve` result carries only its own resolution: it is not
                    // module info.
                    const resolved = await this.resolve('virtual:with-meta', id, {
                      skipSelf: false,
                    });
                    seen.resolveMeta = { ...resolved!.meta, side: resolved!.moduleSideEffects };
                    break;
                  }
                  case 'reads-own-side-effects.js':
                    seen.ownSideEffects = this.getModuleInfo(id)!.moduleSideEffects;
                    break;
                }
                return null;
              },
            },
            generateBundle() {
              seen.ownMeta = { ...this.getModuleInfo(path.join(src, 'writes-own-meta.js'))!.meta };
            },
          },
        ],
      );

      expect(clean.seen).toEqual({
        resolveId: 1,
        readsMeta: 1,
        readsSideEffects: true,
        otherInfo: [true, 8],
        resolveMeta: { r: 1, side: true },
        ownSideEffects: true,
        ownMeta: { own: true },
      });
      expect(restart.calls.sort()).toEqual([
        'assigns-side-effects.js',
        'main.js',
        'reads-meta.js',
        'reads-own-side-effects.js',
        'reads-side-effects.js',
        'writes-own-meta.js',
      ]);
      expect(first.seen).toEqual(clean.seen);
      // The hooks of the reused modules do not run, so they add nothing to `seen`.
      expect(restart.seen.ownMeta).toEqual(clean.seen.ownMeta);
      expect(restart.output).toEqual(clean.output);
    });

    describe('dev engine', () => {
      const main = () => path.join(src, 'main.js');

      /** Starts a dev engine and waits for its first output. */
      async function startDev(
        plugins: Plugin[],
        options: { lazy?: boolean; rebuildStrategy?: 'always' } = {},
      ) {
        const outputs: RolldownOutput[] = [];
        const engine = await dev(
          {
            input: main(),
            cwd: root,
            plugins,
            experimental: {
              cache: { dir: cacheDir },
              devMode: options.lazy ? { lazy: true } : undefined,
            },
          },
          { dir: path.join(root, 'dist') },
          {
            rebuildStrategy: options.rebuildStrategy,
            watch: { watcher: getDevWatchOptionsForCi().watcher },
            onOutput: (result) => {
              if (!(result instanceof Error)) outputs.push(result);
            },
          },
        );
        await engine.run();
        return { engine, outputs };
      }

      const outputCode = (outputs: RolldownOutput[]) =>
        outputs
          .at(-1)
          ?.output.map((chunk) => (chunk.type === 'chunk' ? chunk.code : ''))
          .join('\n');

      test('a partial scan starts a changed module from its saved creating resolution', async () => {
        writeFiles({
          'main.js': `import { dep } from './dep.js';\nconsole.log(dep);\n`,
          'dep.js': `export const dep = 'v1';\n`,
        });
        const dep = path.join(src, 'dep.js');
        const makePlugins = (calls: string[], metas: unknown[]): Plugin[] => [
          {
            name: 'dev-meta',
            resolveId: {
              cache: true,
              handler(source, importer) {
                if (source !== './dep.js' || !importer) return null;
                calls.push('resolveId');
                return { id: dep, meta: { from: 'resolveId' } };
              },
            },
            transform: {
              cache: true,
              handler(code, id) {
                if (id !== dep) return null;
                calls.push('transform');
                return { code, meta: { own: /'(v\d)'/.exec(code)![1] } };
              },
            },
            moduleParsed(info) {
              if (info.id === dep) metas.push({ ...info.meta });
            },
          },
        ];

        const firstRun = await startDev(makePlugins([], []), { rebuildStrategy: 'always' });
        await vi.waitFor(() => expect(firstRun.outputs.length).toBeGreaterThan(0));
        await firstRun.engine.close();

        const calls: string[] = [];
        const metas: unknown[] = [];
        const { engine, outputs } = await startDev(makePlugins(calls, metas), {
          rebuildStrategy: 'always',
        });
        try {
          await vi.waitFor(() => expect(outputs.length).toBeGreaterThan(0));
          expect(calls).toEqual([]);
          expect(metas).toEqual([{ from: 'resolveId', own: 'v1' }]);

          // The watcher compares whole seconds.
          await sleep(1000);
          fs.writeFileSync(dep, `export const dep = 'v2';\n`);
          await vi.waitFor(
            async () => {
              await engine.ensureLatestBuildOutput();
              expect(outputCode(outputs)).toContain('v2');
            },
            { timeout: 20_000 },
          );
          // `main.js` does not resolve `dep.js` again. The watcher can start more than one
          // rebuild for the edit, and a partial scan reads no saved result.
          expect(calls.length).toBeGreaterThan(0);
          expect(calls.every((call) => call === 'transform')).toBe(true);
          expect(metas.at(-1)).toEqual({ from: 'resolveId', own: 'v2' });
        } finally {
          await engine.close();
        }
      }, 60_000);

      test('a lazy compile reuses the saved result of the real module', async () => {
        writeFiles({
          'main.js': `globalThis.load = () => import('./lazy.js');\n`,
          'lazy.js': `export const value = 'LAZY_VALUE';\n`,
        });
        const lazy = path.join(src, 'lazy.js');
        const makePlugins = (calls: string[], seen: unknown[]): Plugin[] => [
          {
            name: 'lazy-meta',
            resolveId: {
              cache: true,
              handler(source, importer) {
                if (source !== './lazy.js' || importer !== main()) return null;
                return { id: lazy, meta: { from: 'resolveId' }, moduleSideEffects: false };
              },
            },
            transform: {
              cache: true,
              handler(code, id) {
                if (id !== lazy) return null;
                calls.push('transform');
                return { code, meta: { own: 'transform' }, moduleSideEffects: 'no-treeshake' };
              },
            },
            moduleParsed(info) {
              if (info.id === lazy) {
                seen.push({ meta: { ...info.meta }, moduleSideEffects: info.moduleSideEffects });
              }
            },
          },
        ];
        const compile = async (calls: string[], seen: unknown[]) => {
          const { engine } = await startDev(makePlugins(calls, seen), { lazy: true });
          try {
            await engine.registerClient('c1');
            return (await engine.compileEntry(`${lazy}?rolldown-lazy=1`, 'c1')).code;
          } finally {
            await engine.close();
          }
        };

        const firstCalls: string[] = [];
        const firstSeen: unknown[] = [];
        expect(await compile(firstCalls, firstSeen)).toContain('LAZY_VALUE');
        expect(firstCalls).toEqual(['transform']);
        expect(firstSeen).toEqual([
          { meta: { from: 'resolveId', own: 'transform' }, moduleSideEffects: 'no-treeshake' },
        ]);

        const calls: string[] = [];
        const seen: unknown[] = [];
        expect(await compile(calls, seen)).toContain('LAZY_VALUE');
        expect(calls).toEqual([]);
        expect(seen).toEqual(firstSeen);
      }, 60_000);
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

  test('a log that changes when it is saved as JSON gives its module no saved result', async () => {
    writeFiles(BASIC_FILES);
    const makePlugins = (calls: string[]): Plugin[] => [
      recordingPlugin(calls, true),
      {
        name: 'warner',
        transform: {
          cache: true,
          handler(_code, id) {
            if (id === path.join(src, 'a.js')) {
              this.warn({ message: 'look at a', cause: new Error('the cause') });
            } else if (id === path.join(src, 'b.js')) {
              this.warn('look at b');
            }
            return null;
          },
        },
      },
    ];
    const collect = () => {
      const logs: string[] = [];
      const onLog: InputOptions['onLog'] = (_level, log) => {
        if (log.plugin === 'warner') {
          logs.push(`${log.message}|${(log as { cause?: Error }).cause?.message ?? ''}`);
        }
      };
      return { logs, onLog };
    };

    const first = collect();
    await build({ plugins: makePlugins([]), onLog: first.onLog });
    for (let i = 0; i < 2; i++) {
      const again = collect();
      const calls: string[] = [];
      await build({ plugins: makePlugins(calls), onLog: again.onLog });
      // `a.js` runs its hooks again, so its log keeps the `Error`. `b.js` reuses its saved result.
      expect(calls.sort()).toEqual([
        'recording.load:a.js',
        'recording.resolveId:a.js:./c.js',
        'recording.transform:a.js',
      ]);
      expect(again.logs.sort()).toEqual(first.logs.sort());
    }
    expect(first.logs.sort()).toEqual([
      expect.stringMatching(/look at a.*\|the cause$/),
      expect.stringMatching(/look at b.*\|$/),
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

  test('the `declaredModuleSize` that a cached `transform` returns is read again after a restart', async () => {
    writeFiles(BASIC_FILES);
    const declareSize = (calls: string[]): Plugin => ({
      name: 'declare-size',
      transform: {
        cache: true,
        handler(code, id) {
          if (!id.startsWith(src)) return null;
          calls.push(`transform:${rel(id)}`);
          return rel(id) === 'b.js' ? { code, declaredModuleSize: 1001 } : null;
        },
      },
    });
    const output: OutputOptions = {
      codeSplitting: { groups: [{ name: 'small', test: /b\.js$/, maxModuleSize: 1000 }] },
    };
    const hasSmallChunk = (files: Awaited<ReturnType<typeof build>>) =>
      files.some((file) => file.fileName.startsWith('small'));
    expect(hasSmallChunk(await build({ plugins: [], cache: undefined, output }))).toBe(true);

    const clean = await build({ plugins: [declareSize([])], cache: undefined, output });
    expect(hasSmallChunk(clean)).toBe(false);
    const first = await build({ plugins: [declareSize([])], output });
    const calls: string[] = [];
    const restart = await build({ plugins: [declareSize(calls)], output });
    expect(calls).toEqual([]);
    expect(first).toEqual(clean);
    expect(restart).toEqual(clean);
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
