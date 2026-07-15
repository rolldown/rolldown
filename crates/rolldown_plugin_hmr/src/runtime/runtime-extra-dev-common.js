// @ts-check

class Module {
  /**
   * @type {{ exports: any }}
   */
  exportsHolder = { exports: null };
  /**
   * @type {string}
   */
  id;

  /**
   * @param {string} id
   */
  constructor(id) {
    this.id = id;
  }

  get exports() {
    return this.exportsHolder.exports;
  }
}

/**
 * Compiler-emitted module-graph delta — pure topology (static + dynamic edges).
 * `ids[0, localCount)` are the modules this payload carries; `ids[localCount, …)` are foreign edge targets.
 * `edges[i]` / `dynamicEdges[i]` are the static / dynamic-`import()` out-edges of `ids[i]`.
 * @typedef {{ ids: string[], localCount: number, edges: number[][], dynamicEdges?: number[][] }} ModuleGraphDelta
 * @typedef {{ createModuleHotContext(moduleId: string): any, onModuleCacheRemoval(moduleId: string): void }} DevRuntimeHooks
 */

export class MissingFactoryError extends Error {
  /**
   * @param {string} id
   */
  constructor(id) {
    super(`No factory registered for module ${id}`);
    this.id = id;
  }
}

export class DevRuntime {
  /**
   * Client ID generated at runtime initialization, used for lazy compilation requests.
   * @type {string}
   */
  clientId;

  /**
   * @param {string} clientId
   */
  constructor(clientId) {
    this.clientId = clientId;
  }

  /**
   * Static import edges from `registerGraph` — entries persist across `removeModuleCache`
   * and change only by replacement from a newer payload (last write wins).
   * @type {Map<string, { edges: string[] }>}
   */
  staticImports = new Map();
  /**
   * Reverse index over the static imports.
   * @type {Map<string, Set<string>>}
   */
  importers = new Map();
  /**
   * Dynamic `import()` edges from `registerGraph`, keyed by importer — mirror of
   * `staticImports` for the dynamic reverse index.
   * @type {Map<string, { edges: string[] }>}
   */
  dynamicImports = new Map();
  /**
   * Reverse index over the dynamic imports.
   * @type {Map<string, Set<string>>}
   */
  dynamicImporters = new Map();
  /**
   * The module cache. Membership means "this module's side effects ran in this tab" —
   * registration is emitted ahead of every module body, and nothing un-registers on
   * unwind, so a factory that throws mid-body stays registered. A `Map` rather than a
   * plain object: HMR eviction deletes entries, and a `delete` on an object drops V8
   * into dictionary mode, taxing every later lookup on the hottest read path.
   * @type {Map<string, Module>}
   */
  moduleCache = new Map();
  /**
   * Re-runnable factories from HMR patches and lazy chunks. The initial bundle stays
   * scope-hoisted and contributes none.
   * @type {Map<string, { kind: 'esm' | 'cjs', fn: (id: string) => void }>}
   */
  factories = new Map();
  /**
   * Installed by the dev client at boot. The runtime is a store + executor and makes
   * no HMR decisions; accepting, disposing, and reloading live behind these hooks.
   * @type {DevRuntimeHooks | null}
   */
  hooks = null;

  /**
   * @param {ModuleGraphDelta} delta
   */
  registerGraph(delta) {
    for (let i = 0; i < delta.localCount; i++) {
      const id = delta.ids[i];
      const edges = delta.edges[i].map((j) => delta.ids[j]);
      for (const target of this.staticImports.get(id)?.edges ?? []) {
        this.importers.get(target)?.delete(id);
      }
      for (const target of edges) {
        let importerSet = this.importers.get(target);
        if (!importerSet) {
          importerSet = new Set();
          this.importers.set(target, importerSet);
        }
        importerSet.add(id);
      }
      this.staticImports.set(id, { edges });

      // Dynamic `import()` edges are maintained in a parallel reverse index with the same
      // last-write-wins bookkeeping; `getImporters` unions the two.
      const dynamicEdges = (delta.dynamicEdges?.[i] ?? []).map((j) => delta.ids[j]);
      for (const target of this.dynamicImports.get(id)?.edges ?? []) {
        this.dynamicImporters.get(target)?.delete(id);
      }
      for (const target of dynamicEdges) {
        let importerSet = this.dynamicImporters.get(target);
        if (!importerSet) {
          importerSet = new Set();
          this.dynamicImporters.set(target, importerSet);
        }
        importerSet.add(id);
      }
      this.dynamicImports.set(id, { edges: dynamicEdges });
    }
  }

  /**
   * @param {string} id
   * @param {'esm' | 'cjs'} kind
   * @param {(id: string) => void} fn
   */
  registerFactory(id, kind, fn) {
    this.factories.set(id, { kind, fn });
  }

  /**
   * @param {string} id
   * @param {{ exports: any }} exportsHolder
   */
  registerModule(id, exportsHolder) {
    const module = new Module(id);
    module.exportsHolder = exportsHolder;
    this.moduleCache.set(id, module);
  }

  /**
   * @param {string} id
   * @returns {string[]}
   */
  getImporters(id) {
    // Static ∪ dynamic importers — the boundary walk treats both kinds the same (parity
    // with Vite `node.importers` / webpack `module.parents`). Deduped so a module that
    // imports `id` both statically and via `import()` appears once.
    const dynamic = this.dynamicImporters.get(id);
    if (!dynamic || dynamic.size === 0) {
      return [...(this.importers.get(id) ?? [])];
    }
    return [...new Set([...(this.importers.get(id) ?? []), ...dynamic])];
  }

  /**
   * @param {string} id
   */
  isExecuted(id) {
    return this.moduleCache.has(id);
  }

  /**
   * @param {string} id
   */
  hasFactory(id) {
    return this.factories.has(id);
  }

  /**
   * Module-cache delete only — static imports and factories persist. Removal is what
   * re-arms a cache-gated factory for `initModule`.
   * @param {string} id
   */
  removeModuleCache(id) {
    this.moduleCache.delete(id);
    this.hooks?.onModuleCacheRemoval(id);
  }

  /**
   * The one re-execution gate: registered → return the live exports; otherwise run the
   * mapped factory (which registers itself first, then runs the body).
   * @param {string} id
   */
  initModule(id) {
    if (this.moduleCache.has(id)) {
      return this.loadExports(id);
    }
    const factory = this.factories.get(id);
    if (!factory) {
      throw new MissingFactoryError(id);
    }
    factory.fn(id);
    return this.loadExports(id);
  }

  /**
   * @param {string} id
   */
  loadExports(id) {
    const module = this.moduleCache.get(id);
    if (module) {
      return module.exportsHolder.exports;
    } else {
      console.warn(`Module ${id} not found`);
      return {};
    }
  }

  /**
   * @param {string} moduleId
   */
  createModuleHotContext(moduleId) {
    if (this.hooks) {
      return this.hooks.createModuleHotContext(moduleId);
    }
    throw new Error('createModuleHotContext requires installed hooks or an override');
  }

  /** @internal */
  // @ts-expect-error The variable will be injected at build time.
  __toESM = __toESM;
  /** @internal */
  // @ts-expect-error The variable will be injected at build time.
  __toCommonJS = __toCommonJS;
  /** @internal */
  // @ts-expect-error The variable will be injected at build time.
  __exportAll = __exportAll;
  /**
   * @param {boolean} [isNodeMode]
   * @returns {(mod: any) => any}
   * @internal
   */
  // @ts-expect-error The variable will be injected at build time.
  __toDynamicImportESM = (isNodeMode) => (mod) => __toESM(mod.default, isNodeMode);
  /** @internal */
  // @ts-expect-error The variable will be injected at build time.
  __reExport = __reExport;
}
