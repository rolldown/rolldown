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
   * A CommonJS module registers its raw `module.exports`; `requestLazy` builds the `__toESM`
   * view on top. ESM registers its namespace, used as is.
   * @type {boolean}
   */
  isCommonJs = false;
  /**
   * The session number, when the module had one as it registered. A number never changes.
   * @type {number | undefined}
   */
  num = undefined;

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
 * Compiler-emitted module-graph delta — topology (static + dynamic edges).
 * `ids[0, localCount)` are the modules this payload carries; `ids[localCount, …)` are foreign edge targets.
 * `edges[i]` / `dynamicEdges[i]` are the static / dynamic-`import()` out-edges of `ids[i]`.
 * `bindings[i][j]` are the export names `ids[i]` imports through `edges[i][j]`. `bindings` holds
 * only some rows; a missing row, or a missing or `null` entry, means the whole namespace.
 * `dynamicEdges` also holds only some rows; a missing row means no dynamic edges.
 * `nums[i]` is the session number of `ids[i]`, foreign edge targets included. Requests name
 * modules by these numbers.
 * `epoch` tells dev engines apart. Only full-build rows carry it. A payload's rows get the
 * payload's generation from `beginPayload`. The generation of a full-build output comes from
 * `setOutputGeneration`.
 * `entries` lists the rows whose module is an entry.
 * @typedef {{ ids: string[], localCount: number, edges: number[][], bindings?: Record<number, (string[] | null)[]>, dynamicEdges?: Record<number, number[]>, nums?: number[], epoch?: number, entries?: number[] }} ModuleGraphDelta
 * @typedef {{ createModuleHotContext(moduleId: string): any, onModuleCacheRemoval(moduleId: string): void, onMissingFactory?(moduleId: string): void, onReloadNeeded?(reason: string): void, onCaughtUp?(): void }} DevRuntimeHooks
 *
 * One update a client missed, sent at the start of a payload when the server is at a newer
 * generation than the request's `g`. It acts like the push the client did not get. A missing
 * `changedIds` is empty.
 * @typedef {{ gen: number, changedIds?: string[], staleIds: string[] }} HmrMissedUpdate
 *
 * The object `beginPayload` returns. A payload calls these instead of the runtime's own methods,
 * so its rows, factories and catch-up all carry the payload's generation.
 * @typedef {{ catchUp(missed: HmrMissedUpdate[]): void, registerGraph(delta: ModuleGraphDelta): void, registerFactory(id: string, fn: (id: string) => void): void }} PayloadScope
 *
 * What `ensureRunnable` found. `caught-up`: a payload brought news of updates the client
 * missed, so the caller walks again with `changedIds` added to its changed modules.
 * @typedef {{ type: 'ready' } | { type: 'caught-up', changedIds: string[] } | { type: 'reload', reason: string }} EnsureRunnableResult
 */

/**
 * @param {Uint8Array} bytes
 * @param {number} length
 * @returns {string}
 */
function toBase64Url(bytes, length) {
  let binary = '';
  for (let i = 0; i < length; i += 0x2000) {
    binary += String.fromCharCode.apply(null, Array.from(bytes.subarray(i, Math.min(i + 0x2000, length))));
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/**
 * A set of module numbers in the request format: `b` and a bitset in base64url without padding.
 * Bit `n` is set for number `n`, bytes in little-endian bit order. An empty set is `''`. See
 * internal-docs/hmr/design.md.
 */
class NumSet {
  /**
   * @param {number} maxNum the highest number that can be added
   */
  constructor(maxNum) {
    this.bits = new Uint8Array((maxNum >> 3) + 1);
    /** The bytes up to the last nonzero one. */
    this.length = 0;
  }

  /**
   * @param {number | undefined} num
   */
  add(num) {
    // The server treats a module with no number as not executed and not held, which can only
    // add modules to the payload.
    if (num === undefined) return;
    const byte = num >> 3;
    this.bits[byte] |= 1 << (num & 7);
    if (byte >= this.length) this.length = byte + 1;
  }

  /** @returns {string} */
  encode() {
    return this.length === 0 ? '' : `b${toBase64Url(this.bits, this.length)}`;
  }
}

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
   * An id the dev client may use in its own messages. The runtime does not use it.
   * @type {string}
   */
  clientId;

  /**
   * @param {string} [clientId]
   */
  constructor(clientId = '') {
    this.clientId = clientId;
  }

  /**
   * Static import edges from `registerGraph` — entries persist across `removeModuleCache`
   * and change only by replacement from a newer payload (last write wins).
   * `bindings` is the payload row as sent; `getImportedBindings` resolves a missing entry.
   * @type {Map<string, { edges: string[], bindings: (string[] | null)[] | undefined }>}
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
  /** @type {Set<string>} */
  entries = new Set();
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
   * @type {Map<string, (id: string) => void>}
   */
  factories = new Map();
  /**
   * The ids of `factories` whose module is not in the module cache. A request names only these
   * factories and those of the update, and there are few of them.
   * @type {Set<string>}
   */
  factoriesNotRun = new Set();
  /**
   * The generation of the payload whose row each id has. A late payload (a slow lazy chunk after
   * an HMR patch) must not replace a row from a newer one. Full-build rows have no entry.
   * @type {Map<string, number>}
   */
  rowGens = new Map();
  /**
   * For each id, the larger of the generation of the installed factory and the floor that
   * `markStale` set, so a payload rendered before an update cannot install an old factory. It is
   * kept apart from `rowGens`: a full-build row must not lower this floor.
   * @type {Map<string, number>}
   */
  factoryGens = new Map();
  /** The generation of the newest payload that ran. */
  lastPayloadGen = -1;
  /**
   * Session numbers of the modules, from `registerGraph`. A request names modules by them.
   * @type {Map<string, number>}
   */
  nums = new Map();
  /** The highest number in `nums`. */
  maxNum = -1;
  /**
   * The epoch of the dev engine whose output this page runs. Set by the first full-build rows.
   * @type {number | undefined}
   */
  epoch = undefined;
  /**
   * The newest generation up to which this client heard of every update. Every factory it holds
   * is fresh as of this generation. It starts at the generation of the output the page loaded
   * (`setOutputGeneration`). A push raises it only to the next generation, and a catch-up to the
   * server's generation. -1 means the client does not know the generation of its output: then a
   * request leaves out `g`, and the server uses the generation of the output it serves.
   */
  knownGen = 0;
  /**
   * The newest generation a push announced. When it is above `knownGen`, the client missed an
   * update, and `ensureRunnable` asks the server for the catch-up.
   */
  pushedGen = 0;
  /**
   * The newest generation whose changes the client walked and applied. A push at or below it
   * only marks modules stale.
   */
  appliedGen = 0;
  /**
   * Changed ids from catch-ups that no walk handled yet, each with the generation of the update
   * that changed it. `ensureRunnable` hands them to its caller.
   * @type {Map<string, number>}
   */
  caughtUpChangedIds = new Map();
  /**
   * Modules whose factory `markStale` dropped, that did not run, and that a module which ran
   * imports statically. A `require()` inside a function runs such a module later, and it cannot
   * wait for a request. So the next pull asks for them again (`refetchDropped` when no update
   * pulls).
   * @type {Set<string>}
   */
  droppedDeps = new Set();
  /** @type {Promise<void> | undefined} */
  refetching = undefined;
  /** Makes each request URL unique: the browser keeps one module record per URL. */
  requestNonce = 0;
  /**
   * Counts `registerModule` calls. A module that ran while the HMR client waited may import a
   * module of the update, so the client walks again when the count changed.
   */
  registerCount = 0;
  /**
   * Set when the server answered a request with a reload. The server cannot serve this client
   * any more, so nothing requests again.
   * @type {string | undefined}
   */
  reloadReason = undefined;
  /**
   * Installed by the dev client at boot. The runtime is a store + executor and makes
   * no HMR decisions; accepting, disposing, and reloading live behind these hooks.
   * @type {DevRuntimeHooks | null}
   */
  hooks = null;

  /**
   * The first statement of every payload: `const __rolldown_payload__ =
   * __rolldown_runtime__.beginPayload(gen)`. `gen` is the server's generation when it rendered
   * the payload, and every row, factory and catch-up of the payload is as of `gen`.
   * @param {number} gen
   * @returns {PayloadScope}
   */
  beginPayload(gen) {
    // A client with no known generation asked without `g`; this payload answers as of `gen`.
    if (this.knownGen < 0) this.knownGen = gen;
    if (gen > this.lastPayloadGen) this.lastPayloadGen = gen;
    return {
      catchUp: (missed) => this.catchUp(gen, missed),
      registerGraph: (delta) => this.registerGraph(delta, gen),
      registerFactory: (id, fn) => this.registerFactory(id, fn, gen),
    };
  }

  /**
   * Full-build chunks call this with no `gen`. Their code runs right away, so their rows always
   * replace the old ones. A payload's rows (through `beginPayload`) replace only rows of an
   * older or the same generation.
   * @param {ModuleGraphDelta} delta
   * @param {number} [gen]
   */
  registerGraph(delta, gen) {
    // Gen 0 is a real payload generation, so test for `undefined`, not for a falsy value.
    const isBuildRows = gen === undefined;
    if (isBuildRows && this.epoch === undefined && delta.epoch !== undefined) {
      // The first full-build rows describe the output this page loaded.
      this.epoch = delta.epoch;
    }
    // A number never changes for an id, so a row dropped below as older still gives its number.
    delta.nums?.forEach((num, i) => {
      this.nums.set(delta.ids[i], num);
      if (num > this.maxNum) this.maxNum = num;
    });
    for (let i = 0; i < delta.localCount; i++) {
      const id = delta.ids[i];
      if (isBuildRows) {
        this.rowGens.delete(id);
      } else {
        if (gen < (this.rowGens.get(id) ?? 0)) continue;
        this.rowGens.set(id, gen);
      }
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
      this.staticImports.set(id, { edges, bindings: delta.bindings?.[i] });

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

      if (delta.entries?.includes(i)) {
        this.entries.add(id);
      } else {
        this.entries.delete(id);
      }
    }
  }

  /**
   * Payloads call this through `beginPayload`, which passes the payload's generation.
   * @param {string} id
   * @param {(id: string) => void} fn
   * @param {number} [gen]
   */
  registerFactory(id, fn, gen = 0) {
    if (gen < (this.factoryGens.get(id) ?? 0)) return;
    this.factoryGens.set(id, gen);
    this.factories.set(id, fn);
    if (!this.moduleCache.has(id)) this.factoriesNotRun.add(id);
  }

  /**
   * @param {string} id
   * @param {{ exports: any }} [exportsHolder]
   * @param {boolean | 0 | 1} [isCommonJs]
   */
  registerModule(id, exportsHolder = { exports: {} }, isCommonJs = false) {
    const previous = this.moduleCache.get(id);
    // A factory runs only for a module that is not in the cache, so a full-build chunk runs
    // this module again. See internal-docs/hmr/design.md (plain `import()`). Vite's module
    // runner can register an external again with the same exports, which is harmless.
    if (previous && previous.exports !== exportsHolder.exports) {
      console.warn(
        `[rolldown] Module ${id} ran a second time. Modules that imported it before keep the first instance.`,
      );
    }
    const module = new Module(id);
    module.num = this.nums.get(id);
    module.exportsHolder = exportsHolder;
    module.isCommonJs = !!isCommonJs;
    this.moduleCache.set(id, module);
    this.factoriesNotRun.delete(id);
    this.registerCount++;
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
   * `"*"` means the whole namespace, an empty list a side-effect-only import, `undefined` no edge.
   * @param {string} importer
   * @param {string} id
   * @returns {string[] | undefined}
   */
  getImportedBindings(importer, id) {
    const record = this.staticImports.get(importer);
    const j = record ? record.edges.indexOf(id) : -1;
    const names = j === -1 ? undefined : (record?.bindings?.[j] ?? ['*']);
    if (!this.dynamicImports.get(importer)?.edges.includes(id)) {
      return names;
    }
    if (!names) return ['*'];
    return names.includes('*') ? names : [...names, '*'];
  }

  /**
   * @param {string} id
   */
  isEntry(id) {
    return this.entries.has(id);
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
    if (this.factories.has(id)) this.factoriesNotRun.add(id);
    // `registerModule` installs a fresh namespace on every run, so a kept memo would hand a
    // later `import()` the pre-edit exports forever.
    this.lazyRequests.delete(id);
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
      this.hooks?.onMissingFactory?.(id);
      throw new MissingFactoryError(id);
    }
    factory(id);
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

  /** @type {Map<string, Promise<any>>} */
  lazyRequests = new Map();

  /**
   * The entry point for a lazy `import()`. `id` is the module the boundary stands for;
   * `proxyModuleId` is the boundary's own id, used only to load the chunk.
   *
   * Nothing is registered under the boundary id, deliberately. A cache entry with no factory
   * behind it reads as "executed" to the HMR boundary walk, and `applyUpdate` turns an
   * updated-but-factory-less module into a full page reload.
   *
   * A failed load (a failed request, or a factory still missing after two requests) is not
   * memoized, so the next `import()` asks again, for example after the user fixed the file. An
   * error thrown by the module's body is memoized, matching `import()` of a module that threw:
   * a factory registers its module before running its body, so running `initModule` again would
   * return half-initialized exports as success.
   *
   * A client that missed an update requests first, so it does not run a factory it holds before
   * the catch-up.
   *
   * A CommonJS module resolves to its `__toESM` view, as in a production build. The runtime
   * decides from the registration: the importer is compiled before the importee is in the
   * graph. `isNodeMode` is the importer's choice, so the view is built per call, not memoized.
   *
   * @param {string} id
   * @param {string} proxyModuleId
   * @param {boolean | 0 | 1} [isNodeMode]
   * @returns {Promise<any>}
   */
  requestLazy(id, proxyModuleId, isNodeMode) {
    let pending = this.lazyRequests.get(id);
    if (!pending) {
      // Factories outlive `removeModuleCache`, so an evicted module can be re-run without the
      // server — and must be, since the memo went with the cache entry. `Promise.resolve` keeps
      // a synchronous `initModule` throw from escaping the call site.
      const loaded = Promise.resolve().then(async () => {
        // A chunk can arrive without a module the client dropped while it was on the way.
        // The second request describes the client as it is now.
        for (let loads = 0; loads < 2 && this.needsRequest(id); loads++) {
          this.throwIfReloadNeeded();
          await this.loadLazyChunk(proxyModuleId);
        }
        this.throwIfMissing([id]);
      });
      const started = loaded.then(() => {
        const exports = this.initModule(id);
        this.notifyCaughtUp();
        return exports;
      });
      pending = started;
      loaded.catch(() => {
        if (this.lazyRequests.get(id) === started) this.lazyRequests.delete(id);
      });
      this.lazyRequests.set(id, pending);
    }

    return pending.then((exports) =>
      this.moduleCache.get(id)?.isCommonJs ? this.__toESM(exports, isNodeMode) : exports,
    );
  }

  /**
   * The entry point for a plain (not lazy) `import()` in a factory. Its target is not a static
   * dep, so the payload that carried the importer did not carry it: the client may have never
   * run it nor received its factory. Then the client pulls it, with the target as the update
   * root, and runs it. Like `requestLazy`, it gives up after two pulls and requests a reload, and
   * a failed pull rejects.
   * @param {string} id
   * @returns {Promise<any>}
   */
  loadDynamic(id) {
    if (!this.needsRequest(id)) {
      try {
        return Promise.resolve(this.initModule(id));
      } catch (error) {
        return Promise.reject(error);
      }
    }
    return this.pullAndInit(id);
  }

  /**
   * @internal
   * @param {string} id
   */
  async pullAndInit(id) {
    const roots = new Set([id]);
    for (let pulls = 0; pulls < 2 && this.needsRequest(id); pulls++) {
      this.throwIfReloadNeeded();
      await this.pullPatch(roots);
    }
    this.throwIfMissing(roots);
    const exports = this.initModule(id);
    this.notifyCaughtUp();
    return exports;
  }

  /**
   * Whether `id` needs a request before it runs outside an update: something its run reaches is
   * missing, or the client missed an update, so a factory it holds may be old.
   * @internal
   * @param {string} id
   */
  needsRequest(id) {
    return !this.isRunnableHere(id) || this.missedUpdate();
  }

  /**
   * @internal
   * @param {Iterable<string>} roots
   */
  throwIfMissing(roots) {
    const missing = this.findMissing(roots, new Set(), false);
    if (missing !== undefined) {
      this.throwIfReloadNeeded();
      this.hooks?.onMissingFactory?.(missing);
      throw new MissingFactoryError(missing);
    }
  }

  /**
   * A request outside an update (a lazy chunk, a dynamic import, a refetch) can bring a
   * catch-up. Its changes would wait for the next push, and the page would run their old code
   * until then. So the client is told, and it runs them as an update. Called after the module
   * of the request ran, so the update does not race with it.
   * @internal
   */
  notifyCaughtUp() {
    if (this.hasCaughtUpChanges()) this.hooks?.onCaughtUp?.();
  }

  /**
   * Whether a catch-up brought changes that no walk handled yet. A catch-up can land after an
   * update of the same generation became ready but before it applied. That update runs those
   * changes, so ids at or below `appliedGen` are dropped here.
   */
  hasCaughtUpChanges() {
    for (const [id, gen] of this.caughtUpChangedIds) {
      if (gen <= this.appliedGen) this.caughtUpChangedIds.delete(id);
    }
    return this.caughtUpChangedIds.size > 0;
  }

  /**
   * Whether `id` can run without the server: every module its run reaches through static
   * imports already ran or has a factory. A factory alone is not enough, because a patch
   * can ship a module without the static imports it never needed (#11028).
   * @internal
   * @param {string} id
   */
  isRunnableHere(id) {
    return this.findMissing([id], new Set(), false) === undefined;
  }

  /**
   * Whether the update `updateIds` can run: its modules are removed from the module cache and
   * run again, so they count as not executed.
   * @param {Set<string>} updateIds
   */
  canRun(updateIds) {
    return this.findMissing(updateIds, updateIds, true) === undefined;
  }

  /**
   * The first module that a run of `roots` reaches through static imports and that has no
   * factory. Modules in `updateIds` count as not executed. With `needsRow`, a module with no
   * row also counts as missing: its static imports are unknown.
   * @internal
   * @param {Iterable<string>} roots
   * @param {Set<string>} updateIds
   * @param {boolean} needsRow
   * @returns {string | undefined}
   */
  findMissing(roots, updateIds, needsRow) {
    const seen = new Set(roots);
    for (const current of seen) {
      if (this.moduleCache.has(current) && !updateIds.has(current)) continue;
      const row = this.staticImports.get(current);
      if (!this.factories.has(current) || (needsRow && !row)) return current;
      for (const dep of row?.edges ?? []) seen.add(dep);
    }
    return undefined;
  }

  /**
   * Sets the generation of the full-build output this page loaded. The output code does not
   * carry it, so that an edit and its undo give the same bytes, and the server that served the
   * page tells it (`hmrGeneration` of the dev engine's output). Call it before the first push
   * arrives.
   * @param {number} gen
   */
  setOutputGeneration(gen) {
    if (gen > this.knownGen) this.knownGen = gen;
    if (gen > this.appliedGen) this.appliedGen = gen;
  }

  /**
   * Drops the factories of `ids` that are older than the update at `gen`. `gen` becomes a floor,
   * so a payload rendered before that update cannot install an old copy later. Runs on every
   * push, also when the update needs nothing here.
   *
   * Generations are consecutive: each update and each engine full reload adds 1. So a push that
   * skips a generation means the client missed an update, for example one pushed before it
   * connected. Only the server knows what that update changed. So `knownGen` stays, and
   * `ensureRunnable` requests the catch-up.
   *
   * A push that comes before the page ran its output (no full-build rows yet) does not raise
   * `knownGen` either. The output then runs the code from before the push, so the push counts as
   * missed, and the caller pulls the catch-up after the output ran (`missedUpdate`).
   * @param {string[]} ids
   * @param {number} gen
   */
  markStale(ids, gen) {
    for (const id of ids) {
      this.markStaleAt(id, gen);
    }
    if (gen > this.pushedGen) this.pushedGen = gen;
    if (this.epoch !== undefined && this.knownGen >= 0 && gen === this.knownGen + 1) {
      this.knownGen = gen;
    }
  }

  /**
   * Whether a push announced a generation that the client does not know all updates up to. A
   * client that ran nothing yet has nothing to catch up: a module runner gets pushes before it
   * loads its entry, and the entry then gives the generation.
   */
  missedUpdate() {
    return this.pushedGen > this.knownGen && this.pushedGen > 0 && this.moduleCache.size > 0;
  }

  /**
   * @internal
   * @param {string} id
   * @param {number} gen
   */
  markStaleAt(id, gen) {
    if ((this.factoryGens.get(id) ?? 0) < gen) {
      this.factories.delete(id);
      this.factoriesNotRun.delete(id);
      this.factoryGens.set(id, gen);
      if (this.isDroppedDep(id)) this.droppedDeps.add(id);
    }
  }

  /**
   * A module with no factory that did not run, while a module that ran imports it statically. A
   * `require()` inside a function can run it later, and it cannot wait for a request.
   * @internal
   * @param {string} id
   */
  isDroppedDep(id) {
    return !this.factories.has(id) && !this.moduleCache.has(id) && this.hasExecutedImporter(id);
  }

  /**
   * @internal
   * @param {string} id
   */
  hasExecutedImporter(id) {
    for (const importer of this.importers.get(id) ?? []) {
      if (this.moduleCache.has(importer)) return true;
    }
    return false;
  }

  /**
   * Pulls the dropped factories that no update pull carried (`droppedDeps`). At most one such
   * pull runs at a time. A failed pull is ignored: the module then reloads the page when it runs,
   * as before.
   * @returns {Promise<void>}
   */
  refetchDropped() {
    if (this.refetching || this.droppedDeps.size === 0 || this.reloadReason !== undefined) {
      return this.refetching ?? Promise.resolve();
    }
    const refetching = (async () => {
      // A second run only when the first one got a newer payload and still lacks a module: a
      // newer push dropped it again while the payload was on the way.
      for (let runs = 0; runs < 2; runs++) {
        const genBefore = this.lastPayloadGen;
        const putBack = await this.pullPatch(new Set()).then(
          (putBack) => {
            this.notifyCaughtUp();
            return putBack;
          },
          () => 0,
        );
        if (putBack === 0 || this.lastPayloadGen <= genBefore || this.reloadReason !== undefined) {
          break;
        }
      }
    })().finally(() => {
      this.refetching = undefined;
    });
    this.refetching = refetching;
    return refetching;
  }

  /**
   * Called by a payload rendered at generation `gen`, newer than the request's `g`, through
   * `beginPayload`. It runs before the payload's rows and factories. Each entry acts like the
   * push the client missed.
   * @param {number} gen
   * @param {HmrMissedUpdate[]} missed
   */
  catchUp(gen, missed) {
    // Compared with `appliedGen`, not `knownGen`: a push may have raised `knownGen` before its
    // walk ran, and the walk must still see these changes.
    if (gen <= this.appliedGen) return;
    for (const entry of missed) {
      for (const id of entry.staleIds) this.markStaleAt(id, entry.gen);
      if (entry.gen > this.appliedGen) {
        for (const id of entry.changedIds ?? []) {
          if (entry.gen > (this.caughtUpChangedIds.get(id) ?? 0)) {
            this.caughtUpChangedIds.set(id, entry.gen);
          }
        }
      }
    }
    if (gen > this.knownGen) this.knownGen = gen;
  }

  /**
   * Makes sure the update `updateIds` can run, pulling the missing factories from the server
   * if needed. Call it before anything is removed from the module cache.
   * @param {Set<string>} updateIds
   * @returns {Promise<EnsureRunnableResult>}
   */
  async ensureRunnable(updateIds) {
    for (let pulls = 0; ; pulls++) {
      if (this.reloadReason !== undefined) return { type: 'reload', reason: this.reloadReason };
      // A catch-up can also come with a lazy chunk. Its changes join the next update, so the
      // client can set `appliedGen` to `knownGen` after it applies.
      if (this.hasCaughtUpChanges()) {
        const changedIds = [...this.caughtUpChangedIds.keys()];
        this.caughtUpChangedIds.clear();
        return { type: 'caught-up', changedIds };
      }
      const missing = this.findMissing(updateIds, updateIds, true);
      // A missed update can change modules that this update runs or that stay, so the client
      // asks for the catch-up even when it can run the update.
      const missedUpdate = this.missedUpdate();
      if (missing === undefined && !missedUpdate) return { type: 'ready' };
      if (pulls === 2) {
        const reason =
          missing === undefined
            ? `no catch-up for generation ${this.pushedGen} after ${pulls} requests`
            : `no factory for ${missing} after ${pulls} requests`;
        return { type: 'reload', reason };
      }
      await this.pullPatch(updateIds);
    }
  }

  /**
   * Run by the body that the server sends instead of a payload when it cannot serve this client:
   * the client runs the output of another dev engine, its generation is too old to catch up, or
   * the request is bad. Only a reload fixes that. A failed request would only reject the
   * `import()` that made it.
   * @param {string} reason
   */
  reloadNeeded(reason) {
    if (this.reloadReason !== undefined) return;
    this.reloadReason = reason;
    this.hooks?.onReloadNeeded?.(reason);
  }

  /** @internal */
  throwIfReloadNeeded() {
    if (this.reloadReason !== undefined) {
      throw new Error(`The page needs a full reload: ${this.reloadReason}`);
    }
  }

  /**
   * What the client holds, as the query of a patch or lazy request: `s` epoch, `g` known
   * generation, `u` the update, `e` executed modules minus the update, `f` held factories of
   * modules that are not in `e`, `n` a nonce. The server never reads a factory of a module in
   * `e`: such a module keeps its exports. An empty set is left out, and so is `g` while the
   * client does not know it.
   * @param {Set<string>} updateIds
   */
  encodeHave(updateIds) {
    let query = `s=${this.epoch ?? 0}`;
    if (this.knownGen >= 0) query += `&g=${this.knownGen}`;
    const update = new NumSet(this.maxNum);
    for (const id of updateIds) update.add(this.nums.get(id));
    const executed = new NumSet(this.maxNum);
    for (const module of this.moduleCache.values()) {
      if (!updateIds.has(module.id)) executed.add(module.num ?? this.nums.get(module.id));
    }
    const factories = new NumSet(this.maxNum);
    for (const id of this.factoriesNotRun) factories.add(this.nums.get(id));
    for (const id of updateIds) {
      if (this.factories.has(id)) factories.add(this.nums.get(id));
    }
    const sets = [
      ['u', update.encode()],
      ['e', executed.encode()],
      ['f', factories.encode()],
    ];
    for (const [key, value] of sets) {
      if (value) query += `&${key}=${value}`;
    }
    return `${query}&n=${++this.requestNonce}`;
  }

  /**
   * The URL of a patch, for a query from `encodeHave`. The patch is served next to the output
   * files, so a server with another output base overrides this method.
   * @param {string} query
   * @returns {string}
   */
  patchUrl(query) {
    return `/__rolldown_hmr_patch.js?${query}`;
  }

  /**
   * Pulls the patch that the update `updateIds` needs. `loadDynamic` passes its target here: a
   * root the client never ran is carried with its static deps the same way. The dropped
   * factories of `droppedDeps` are roots too. When the pull settles, with a payload, a reload
   * body or a failure, each dropped module that still has no factory goes back to `droppedDeps`.
   * @internal
   * @param {Set<string>} updateIds
   * @returns {Promise<number>} how many dropped modules went back
   */
  async pullPatch(updateIds) {
    const taken = [...this.droppedDeps];
    this.droppedDeps.clear();
    const roots = taken.length > 0 ? new Set([...updateIds, ...taken]) : updateIds;
    let putBack = 0;
    try {
      await this.importPatch(this.patchUrl(this.encodeHave(roots)));
    } finally {
      for (const id of taken) {
        if (this.isDroppedDep(id)) {
          this.droppedDeps.add(id);
          putBack++;
        }
      }
    }
    return putBack;
  }

  /**
   * @param {string} url
   * @returns {Promise<unknown>}
   */
  importPatch(url) {
    return import(/* @vite-ignore */ url);
  }

  /**
   * The URL of the lazy chunk behind a lazy boundary, for a query from `encodeHave`. The dev
   * server owns the lazy endpoint, so a server that serves it at another URL (for example under
   * a public base path) overrides this method.
   * @param {string} proxyModuleId
   * @param {string} query
   * @returns {string}
   */
  lazyUrl(proxyModuleId, query) {
    return `/@vite/lazy?id=${encodeURIComponent(proxyModuleId)}&${query}`;
  }

  /**
   * Loads the lazy chunk behind a lazy boundary. The chunk only registers factories;
   * `requestLazy` runs the module afterwards.
   * @param {string} proxyModuleId
   * @returns {Promise<unknown>}
   */
  async loadLazyChunk(proxyModuleId) {
    return import(/* @vite-ignore */ this.lazyUrl(proxyModuleId, this.encodeHave(new Set())));
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
  /** @internal */
  // @ts-expect-error The variable will be injected at build time.
  __reExport = __reExport;
}
