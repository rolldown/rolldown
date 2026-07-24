import fs from 'node:fs';
import path from 'node:path';
import { defineDevConfig } from '@rolldown/test-dev-server';

// Mirror of `hmr-hot-update-hook`, but through VITE's plugin contracts: the
// plugins below sit top-level in vite's plugin array, so the bundled-dev
// adapter (vite `bundledDevHmr.ts`) wraps them onto rolldown's `hotUpdate`
// hook. Covered paths:
// - `config.txt`     → vite `hotUpdate` (new contract, `EnvironmentModuleNode`s):
//                      assert node facade + identity (incl. dynamic-import edges),
//                      REPLACE main.js -> dep.js; a second `hotUpdate` plugin then
//                      asserts the SHARED options object (a reassigned `read` is
//                      visible downstream) and the replaced set
// - `invalidate.txt` → vite `hotUpdate` calling `moduleGraph.invalidateModule(dep)`
//                      and returning [] — the buffered module must ship (tailwind pattern)
// - `custom.txt`     → the md-pages pattern: `ctx.read()` returns the edited content,
//                      `environment.hot.send` delivers a custom event, `[]` suppresses;
//                      also asserts facade reads (`getModulesByFile`, `url`/`file`/`info`,
//                      `importedModules` incl. the dynamic edge)
// - `chain.txt`      → legacy `ctx.read` reassignment protocol across TWO
//                      `handleHotUpdate` plugins (the unocss → plugin-vue shape)
// - `reload.txt`     → vite `hotUpdate` calling `moduleGraph.invalidateAll()` —
//                      full rebuild + page reload
// - `widget.js`      → by-file expansion (the SFC pattern): ctx.modules must
//                      contain the file's query-variant module
//                      (`widget.js?part=extra`), and returning ONLY that
//                      sub-module ships it without re-running the base module
// - `suppress.txt`   → legacy `handleHotUpdate` (shared `HmrContext`, mixed nodes):
//                      assert context shape, SUPPRESS the update
const assert = (cond, message) => {
  if (!cond) throw new Error(`[hmr-hot-update-hook-vite] ${message}`);
};

const viteHotUpdatePlugin = {
  name: 'test-vite-hot-update',
  transform: {
    filter: { id: /main\.js$/ },
    handler(_code, id) {
      // Watch the control files so the engine's default mapping points them at main.js.
      this.addWatchFile(path.join(path.dirname(id), 'config.txt'));
      this.addWatchFile(path.join(path.dirname(id), 'invalidate.txt'));
      this.addWatchFile(path.join(path.dirname(id), 'custom.txt'));
      this.addWatchFile(path.join(path.dirname(id), 'reload.txt'));
      return null;
    },
  },
  hotUpdate(ctx) {
    const dir = path.dirname(ctx.file);
    if (ctx.file.endsWith('config.txt')) {
      assert(ctx.type === 'update', `expected type update, got ${ctx.type}`);
      assert(typeof ctx.timestamp === 'number', 'timestamp must be a number');
      assert(typeof ctx.read === 'function', 'read must be a function');
      assert(ctx.server != null, 'server must be provided');
      assert(
        ctx.modules.length === 1 && ctx.modules[0].id.endsWith('main.js'),
        `expected default modules [main.js], got ${JSON.stringify(ctx.modules.map((m) => m.id))}`,
      );
      // node identity: the affected set and the graph hand out the same object
      const byId = this.environment.moduleGraph.getModuleById(
        ctx.modules[0].id,
      );
      assert(byId === ctx.modules[0], 'node identity must hold');
      const dep = this.environment.moduleGraph.getModuleById(
        path.join(dir, 'dep.js'),
      );
      assert(dep != null, 'dep.js must be resolvable from the module graph');
      assert(
        [...dep.importers].some((m) => m.id?.endsWith('main.js')),
        'dep.js importers must contain main.js',
      );
      // dynamic-import edges: dyn.js is only imported via `import()`; its
      // importer must still be visible through the facade
      const dyn = this.environment.moduleGraph.getModuleById(
        path.join(dir, 'dyn.js'),
      );
      assert(dyn != null, 'dyn.js must be resolvable from the module graph');
      assert(
        [...dyn.importers].some((m) => m.id?.endsWith('main.js')),
        'dyn.js importers must contain its dynamic importer main.js',
      );
      // vite threads one shared options object through the whole chain —
      // reassign `read`; the next plugin asserts it sees this function
      const originalRead = ctx.read;
      ctx.read = Object.assign(() => originalRead(), { stamped: true });
      return [dep]; // replace: the patch must ship dep's factory, not main's
    }
    if (ctx.file.endsWith('invalidate.txt')) {
      const dep = this.environment.moduleGraph.getModuleById(
        path.join(dir, 'dep.js'),
      );
      assert(dep != null, 'dep.js must be resolvable from the module graph');
      // tailwind pattern: buffer dep via invalidateModule; return [] so the
      // final set is exactly the buffered module (default main.js would
      // full-reload — main has no self-accept)
      this.environment.moduleGraph.invalidateModule(dep);
      return [];
    }
    if (ctx.file.endsWith('custom.txt')) {
      const graph = this.environment.moduleGraph;
      const depId = path.join(dir, 'dep.js');
      const dep = graph.getModuleById(depId);
      assert(dep != null, 'dep.js must be resolvable from the module graph');
      // facade reads: by-file lookup hands out the SAME node, and the node's
      // plain fields come from the engine registry
      const byFile = graph.getModulesByFile(depId);
      assert(
        byFile != null && byFile.has(dep),
        'getModulesByFile(dep.js) must contain the same node object',
      );
      assert(dep.url === '/dep.js', `expected url /dep.js, got ${dep.url}`);
      assert(dep.file === depId, `expected file ${depId}, got ${dep.file}`);
      assert(
        dep.info != null && dep.info.id === depId,
        'node.info must expose the engine ModuleInfo',
      );
      // importedModules unions static and dynamic edges (main -> dep, main -> dyn)
      const main = graph.getModuleById(path.join(dir, 'main.js'));
      assert(main != null, 'main.js must be resolvable from the module graph');
      const importedIds = [...main.importedModules].map((m) => m.id);
      assert(
        importedIds.some((mid) => mid?.endsWith('dep.js')) &&
          importedIds.some((mid) => mid?.endsWith('dyn.js')),
        `main.importedModules must contain dep.js and dyn.js, got ${JSON.stringify(importedIds)}`,
      );
      // md-pages pattern: read the edited content, run a custom protocol,
      // suppress the default update
      return ctx.read().then((content) => {
        this.environment.hot.send({
          type: 'custom',
          event: 'custom-update',
          data: { content },
        });
        return [];
      });
    }
    if (ctx.file.endsWith('reload.txt')) {
      // invalidateAll maps to a full rebuild + page reload under bundled dev;
      // return [] so the reload is the only visible effect
      this.environment.moduleGraph.invalidateAll();
      return [];
    }
  },
};

// Runs after `viteHotUpdatePlugin`: pins that every vite `hotUpdate` hook in
// the chain receives the SAME options object (cross-plugin mutations like a
// reassigned `read` stay visible) with the module set reconciled to the
// previous plugin's replacement.
const viteSharedOptionsPlugin = {
  name: 'test-vite-shared-options',
  hotUpdate(ctx) {
    if (!ctx.file.endsWith('config.txt')) return;
    assert(
      ctx.read?.stamped === true,
      'hotUpdate options must be one shared object per event — the reassigned read is not visible',
    );
    assert(
      ctx.modules.length === 1 && ctx.modules[0].id?.endsWith('dep.js'),
      `expected the replaced set [dep.js], got ${JSON.stringify(ctx.modules.map((m) => m.id))}`,
    );
  },
};

// Mimics the SFC sub-module pattern (plugin-vue's `App.vue?vue&type=style`):
// `widget.js?part=extra` is a query-variant module whose load() derives its
// content from the base file. On a base-file edit, upstream vite hands hooks
// EVERY module of that file (moduleGraph.getModulesByFile) — the adapter's
// by-file expansion must reproduce that, and returning only the sub-module
// must hot-swap it while the base module does not re-run.
const byFileSubModulePlugin = {
  name: 'test-by-file-sub-module',
  resolveId(source, importer) {
    if (source.endsWith('widget.js?part=extra') && importer) {
      return path.join(path.dirname(importer), 'widget.js?part=extra');
    }
  },
  load(id) {
    if (!id.endsWith('widget.js?part=extra')) return;
    const base = fs.readFileSync(id.split('?')[0], 'utf-8');
    const token = /part-marker: (\S+)/.exec(base)?.[1] ?? 'missing';
    return (
      `document.querySelector('.part').textContent = 'part:${token}';\n` +
      `import.meta.hot.accept();\n`
    );
  },
  hotUpdate(ctx) {
    if (!ctx.file.endsWith('widget.js')) return;
    const ids = ctx.modules.map((m) => m.id);
    assert(
      ids.some((id) => id?.endsWith('/widget.js')),
      `modules must contain the base module, got ${JSON.stringify(ids)}`,
    );
    const sub = ctx.modules.find((m) =>
      m.id?.endsWith('widget.js?part=extra'),
    );
    assert(
      sub != null,
      `by-file expansion must include the query-variant module, got ${JSON.stringify(ids)}`,
    );
    // node identity holds for expanded modules too
    assert(
      this.environment.moduleGraph.getModuleById(sub.id) === sub,
      'expanded sub-module node identity must hold',
    );
    return [sub]; // ship only the sub-module — the base must not re-run
  },
};

// The legacy shared-context protocol (unocss → plugin-vue shape): plugin A
// reads the file through the ORIGINAL ctx.read, then reassigns ctx.read;
// plugin B must observe the reassigned function. B returns [dep] so success
// has a positive browser-side signal (accept count +1) — an assert throw
// aborts the update and the count stays flat.
const legacyReadChainA = {
  name: 'test-legacy-read-chain-a',
  transform: {
    filter: { id: /main\.js$/ },
    handler(_code, id) {
      this.addWatchFile(path.join(path.dirname(id), 'chain.txt'));
      return null;
    },
  },
  async handleHotUpdate(ctx) {
    if (!ctx.file.endsWith('chain.txt')) return;
    const content = await ctx.read();
    assert(
      content.includes('chain-v'),
      `legacy read() must return the file content, got ${JSON.stringify(content)}`,
    );
    ctx.read = async () => 'transformed-by-chain-a';
  },
};

const legacyReadChainB = {
  name: 'test-legacy-read-chain-b',
  async handleHotUpdate(ctx) {
    if (!ctx.file.endsWith('chain.txt')) return;
    assert(
      (await ctx.read()) === 'transformed-by-chain-a',
      'the shared HmrContext must carry plugin A\'s reassigned read',
    );
    const dep = ctx.server.moduleGraph.getModuleById(
      path.join(path.dirname(ctx.file), 'dep.js'),
    );
    assert(dep != null, 'dep.js must be resolvable from the mixed module graph');
    return [dep]; // positive signal: the patch ships dep, accept count +1
  },
};

const legacyHandleHotUpdatePlugin = {
  name: 'test-legacy-handle-hot-update',
  transform: {
    filter: { id: /main\.js$/ },
    handler(_code, id) {
      this.addWatchFile(path.join(path.dirname(id), 'suppress.txt'));
      return null;
    },
  },
  handleHotUpdate(ctx) {
    if (!ctx.file.endsWith('suppress.txt')) return;
    assert(typeof ctx.timestamp === 'number', 'timestamp must be a number');
    assert(typeof ctx.read === 'function', 'read must be a function');
    assert(ctx.server != null, 'server must be provided');
    assert(
      ctx.modules.length === 1 && ctx.modules[0].id?.endsWith('main.js'),
      `expected mixed modules [main.js], got ${JSON.stringify(ctx.modules.map((m) => m.id))}`,
    );
    return []; // suppress: the step must produce a Noop update
  },
};

export default defineDevConfig({
  platform: 'browser',
  build: {
    input: {
      main: 'main.js',
    },
    platform: 'browser',
    treeshake: false,
    experimental: {
      // lazy compilation would route the dynamic import through a
      // `?rolldown-lazy=` stub, and the importer asserts below check the
      // DIRECT graph edges — opt out so dyn.js's importer is main.js itself
      devMode: { lazy: false },
    },
    plugins: [
      viteHotUpdatePlugin,
      viteSharedOptionsPlugin,
      byFileSubModulePlugin,
      legacyReadChainA,
      legacyReadChainB,
      legacyHandleHotUpdatePlugin,
    ],
  },
});
