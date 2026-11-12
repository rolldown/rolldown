import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { type InputOptions, type Plugin, type RolldownLog, rolldown, watch } from 'rolldown';
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
    const { output } = await bundle.generate({ format: 'esm' });
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

  test('modules whose hooks use `meta`, `moduleSideEffects` or `this.load` run on every build', async () => {
    writeFiles({
      'main.js': [
        `import './meta.js';`,
        `import './side-effects-returned.js';`,
        `import './side-effects-assigned.js';`,
        `import './calls-load.js';`,
        `import './calls-load-parsed.js';`,
        `import './plain.js';`,
        `import './loaded.js';`,
        `console.log('main');`,
        ``,
      ].join('\n'),
      'meta.js': `console.log('meta');\n`,
      'side-effects-returned.js': `console.log('side-effects-returned');\n`,
      'side-effects-assigned.js': `console.log('side-effects-assigned');\n`,
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
            if (rel(id) === 'side-effects-returned.js') {
              return { code: fs.readFileSync(id, 'utf8'), moduleSideEffects: true };
            }
            return null;
          },
        },
        transform: {
          cache: true,
          async handler(code, id) {
            if (!id.startsWith(src)) return null;
            calls.push(`transform:${rel(id)}`);
            switch (rel(id)) {
              case 'meta.js':
                return { code, meta: { 'not-cacheable': { value: 1 } } };
              case 'side-effects-assigned.js': {
                // `main.js` is parsed before its imports are transformed.
                const info = this.getModuleInfo(path.join(src, 'main.js'));
                if (!info) throw new Error('no module info for main.js');
                info.moduleSideEffects = true;
                return null;
              }
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
      'load:meta.js',
      'load:side-effects-assigned.js',
      'load:side-effects-returned.js',
      'transform:calls-load-parsed.js',
      'transform:calls-load.js',
      'transform:meta.js',
      'transform:side-effects-assigned.js',
      'transform:side-effects-returned.js',
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

  test('a module whose `resolveId` hook returns `meta` runs on every build', async () => {
    writeFiles(BASIC_FILES);
    const makePlugins = (calls: string[]): Plugin[] => [
      recordingPlugin(calls, true),
      {
        name: 'resolve-meta',
        resolveId: {
          cache: true,
          handler(source, importer) {
            if (importer && source === './c.js') {
              return { id: path.join(src, 'c.js'), meta: { 'resolve-meta': true } };
            }
            return null;
          },
        },
      },
    ];
    await build({ plugins: makePlugins([]) });
    const second: string[] = [];
    await build({ plugins: makePlugins(second) });
    // `a.js` imports `./c.js`, so the task of `a.js` gets no saved result.
    expect(second.sort()).toEqual([
      'recording.load:a.js',
      'recording.resolveId:a.js:./c.js',
      'recording.transform:a.js',
    ]);
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
