import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

const [scenario, rollupModule] = process.argv.slice(2);
const build = rollupModule
  ? (await import(rollupModule)).rollup
  : (await import('rolldown')).rolldown;
const fixture = (path) => fileURLToPath(new URL(path, import.meta.url));

function defer() {
  let resolve;
  const promise = new Promise((r) => (resolve = r));
  return { promise, resolve };
}

// Awaits `this.load` while resolving every import. `@rollup/plugin-commonjs` does the same for
// most imports.
const loadInResolve = {
  name: 'load-in-resolve',
  async resolveId(source, importer, options) {
    const resolved = await this.resolve(source, importer, { ...options, skipSelf: true });
    if (resolved && !resolved.external) {
      const info = await this.load(resolved);
      assert.ok(info.code.includes('value'));
    }
    return resolved;
  },
};

function resolveDependencies() {
  const main = fixture('./resolve-dependencies/main.js');
  const dep = fixture('./resolve-dependencies/dep.js');
  const depImportsResolve = defer();
  return {
    name: 'resolve-dependencies',
    async resolveId(source, importer, options) {
      if (importer === dep) {
        await depImportsResolve.promise;
        return null;
      }
      if (importer !== main) return null;
      const resolved = await this.resolve(source, importer, { ...options, skipSelf: true });
      const parsed = await this.load(resolved);
      // The imports of `dep` wait for `depImportsResolve`.
      assert.deepEqual(parsed.importedIds, []);
      depImportsResolve.resolve();
      const withDependencies = await this.load({ ...resolved, resolveDependencies: true });
      assert.deepEqual(withDependencies.importedIds, [fixture('./dep.js')]);
      return resolved;
    },
  };
}

function entryWaitsForModule() {
  const depTransformed = defer();
  return {
    name: 'entry-waits-for-module',
    async resolveId(source, _importer, options) {
      // `dep.js` is imported by the other entry.
      if (options.isEntry && source === fixture('./other.js')) await depTransformed.promise;
      return null;
    },
    transform(_code, id) {
      if (id === fixture('./dep.js')) depTransformed.resolve();
    },
  };
}

function emittedEntriesWaitForEachOther() {
  const entries = [fixture('./emitted/main.js'), fixture('./other.js')];
  const started = new Map(entries.map((id) => [id, defer()]));
  return {
    name: 'emitted-entries-wait-for-each-other',
    buildStart() {
      for (const id of entries) this.emitFile({ type: 'chunk', id });
    },
    async resolveId(source, _importer, options) {
      if (!options.isEntry || !started.has(source)) return null;
      started.get(source).resolve();
      await started.get(entries.find((id) => id !== source)).promise;
      return null;
    },
  };
}

function entryImportedByAnotherEntry() {
  const dep = fixture('./dep.js');
  const depLoading = defer();
  const isEntry = [];
  return {
    name: 'entry-imported-by-another-entry',
    async resolveId(source, _importer, options) {
      // `input/main.js` imports `dep.js` before `dep.js` is resolved as an entry.
      if (options.isEntry && source === dep) await depLoading.promise;
      return null;
    },
    async load(id) {
      if (id !== dep) return null;
      isEntry.push(this.getModuleInfo(id).isEntry);
      depLoading.resolve();
      // Waits until the entry is resolved, so the module info of `dep.js` changes during its load.
      while (!this.getModuleInfo(id).isEntry) {
        await new Promise((resolve) => setTimeout(resolve, 10));
      }
    },
    transform(_code, id) {
      if (id === dep) isEntry.push(this.getModuleInfo(id).isEntry);
    },
    moduleParsed(info) {
      if (info.id === dep) isEntry.push(info.isEntry);
    },
    buildEnd() {
      assert.deepEqual(isEntry, [false, true, true]);
    },
  };
}

// `other.js` waits for `dep.js`, which `input/main.js` imports after `missing.js` fails to resolve.
function entryError(emitMissing) {
  const depTransformed = defer();
  return {
    name: 'entry-error',
    buildStart() {
      if (emitMissing) this.emitFile({ type: 'chunk', id: fixture('./missing.js') });
    },
    async resolveId(source) {
      if (source === fixture('./missing.js')) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      return null;
    },
    async load(id) {
      if (id === fixture('./input/main.js')) {
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
      if (id === fixture('./other.js')) await depTransformed.promise;
    },
    transform(_code, id) {
      if (id === fixture('./dep.js')) depTransformed.resolve();
    },
  };
}

const scenarios = {
  input: { input: ['./input/main.js'], plugins: [loadInResolve] },
  'input-load-error': {
    input: ['./input/main.js'],
    plugins: [
      loadInResolve,
      {
        name: 'load-error',
        load(id) {
          if (id === fixture('./input/main.js')) {
            throw new Error('fixture entry load failed');
          }
        },
      },
    ],
  },
  emitted: {
    emit: ['./emitted/main.js'],
    plugins: [
      loadInResolve,
      {
        name: 'emit',
        buildStart() {
          this.emitFile({ type: 'chunk', id: fixture('./emitted/main.js') });
        },
      },
    ],
  },
  cycle: { input: ['./cycle/main.js'], plugins: [loadInResolve] },
  'resolve-dependencies': {
    input: ['./resolve-dependencies/main.js'],
    plugins: [resolveDependencies()],
  },
  'entry-waits-for-module': {
    input: ['./input/main.js', './other.js'],
    plugins: [entryWaitsForModule()],
  },
  'emitted-entries-wait-for-each-other': {
    emit: ['./emitted/main.js'],
    plugins: [emittedEntriesWaitForEachOther()],
  },
  'entry-imported-by-another-entry': {
    input: ['./dep.js', './input/main.js'],
    plugins: [entryImportedByAnotherEntry()],
  },
  'input-entry-error': {
    input: ['./missing.js', './input/main.js', './other.js'],
    plugins: [entryError(false)],
  },
  'emitted-entry-error': {
    input: ['./input/main.js', './other.js'],
    plugins: [entryError(true)],
  },
};

const { input = [], emit = [], plugins } = scenarios[scenario];
const entry = fixture([...input, ...emit][0]);
const bundle = await build({
  input: input.map(fixture),
  plugins: [
    ...plugins,
    {
      name: 'check-entry',
      buildEnd(error) {
        if (!error) assert.equal(this.getModuleInfo(entry)?.isEntry, true);
      },
    },
  ],
});
try {
  const { output } = await bundle.generate({ format: 'es' });
  const chunk = output.find((file) => file.type === 'chunk' && file.facadeModuleId === entry);
  assert.ok(chunk);
  const { value } = await import(
    `data:text/javascript;base64,${Buffer.from(chunk.code).toString('base64')}`
  );
  console.log(value);
} finally {
  await bundle.close();
}
