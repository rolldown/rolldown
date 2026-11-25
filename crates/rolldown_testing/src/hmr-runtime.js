// @ts-check

/** @import { DevRuntime } from "../../rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js" */

/** @type {typeof DevRuntime} */
// @ts-expect-error -- there's no way to declare a variable by JSDoc
var BaseDevRuntime = DevRuntime;

class TestHotContext {
  moduleId;
  /** @type {{ deps: string, cb: Function }[]} */
  callbacks = [];
  /** @type {Set<string> | null} */
  acceptedExports = null;
  /** @type {WeakSet<Function>} */
  partialCallbacks = new WeakSet();

  /**
   * @param {string} moduleId
   */
  constructor(moduleId) {
    this.moduleId = moduleId;
  }

  /**
   * Mirrors the shared Vite hot-context surface: a bare `accept()` IS a live
   * self-acceptance under runtime-observed boundaries — the walk reads it.
   * @param {...any} args
   * @returns {void}
   */
  accept(...args) {
    // Like Vite's client, a falsy first argument self-accepts and drops the callback.
    if (typeof args[0] === 'function' || !args[0]) {
      this.callbacks.push({ deps: this.moduleId, cb: args[0] || (() => {}) });
      return;
    }
    this.callbacks.push({ deps: args[0], cb: args[1] ?? (() => {}) });
  }

  /**
   * @param {string | readonly string[]} exportNames
   * @param {Function} [cb]
   */
  acceptExports(exportNames, cb) {
    this.acceptedExports ??= new Set();
    for (const name of typeof exportNames === 'string' ? [exportNames] : exportNames) {
      this.acceptedExports.add(name);
    }
    const callback = cb ?? (() => {});
    this.partialCallbacks.add(callback);
    this.callbacks.push({ deps: this.moduleId, cb: callback });
  }
}

/**
 * Test-harness mirror of the dedicated FBM HMR client. Per push, `__testHandlePush` marks the
 * stale modules, walks the runtime's static imports and module cache with acceptance from the
 * live hot contexts, pulls the factories the update needs (`ensureRunnable`), then removes the
 * update from the module cache and re-runs the boundaries dep-first. A full reload that the
 * client decides on throws: executed fixtures assert hot updates.
 */
class TestDevRuntime extends BaseDevRuntime {
  /** @type {Map<string, TestHotContext>} */
  contexts = new Map();
  /** @type {Set<Promise<unknown>>} */
  pendingDynamicLoads = new Set();

  /**
   * @override
   * @param {string} id
   */
  loadDynamic(id) {
    const pending = super.loadDynamic(id);
    this.pendingDynamicLoads.add(pending);
    const forget = () => this.pendingDynamicLoads.delete(pending);
    pending.then(forget, forget);
    return pending;
  }

  /**
   * Waits for the `import()` calls an update started, and the ones those started, so their
   * pulls belong to the step of that update.
   */
  async __testSettle() {
    for (;;) {
      await new Promise((resolve) => setTimeout(resolve, 0));
      if (this.pendingDynamicLoads.size === 0) return;
      await Promise.allSettled(this.pendingDynamicLoads);
    }
  }

  /**
   * @override
   * @param {string} moduleId
   */
  createModuleHotContext(moduleId) {
    const ctx = new TestHotContext(moduleId);
    this.contexts.set(moduleId, ctx);
    return ctx;
  }

  /**
   * @param {string} id
   * @param {string} dep
   */
  acceptsDepOf(id, dep) {
    const ctx = this.contexts.get(id);
    if (!ctx) return false;
    return ctx.callbacks.some(({ deps }) =>
      Array.isArray(deps) ? deps.includes(dep) : deps === dep,
    );
  }

  /**
   * @param {string} id
   */
  isSelfAccepting(id) {
    const ctx = this.contexts.get(id);
    if (!ctx) return false;
    const plainSelfAccept = ctx.callbacks.some(
      ({ deps, cb }) => deps === id && !ctx.partialCallbacks.has(cb),
    );
    if (plainSelfAccept || !ctx.acceptedExports) return plainSelfAccept;
    const accepted = ctx.acceptedExports;
    return Object.keys(this.loadExports(id)).every((name) => accepted.has(name));
  }

  /**
   * @param {string} id
   * @returns {Set<string> | null}
   */
  acceptedExportsOf(id) {
    return this.contexts.get(id)?.acceptedExports ?? null;
  }

  /**
   * @param {{ changedIds: string[], staleIds: string[], gen: number }} push
   * @returns {Promise<{ status: 'applied' | 'noop' | 'stale-only' | 'reload', pulls: number }>}
   */
  async __testHandlePush({ changedIds, staleIds, gen }) {
    this.markStale(staleIds, gen);
    if (gen <= this.appliedGen) return { status: 'stale-only', pulls: 0 };
    const noncesBefore = this.requestNonce;
    let walkFrom = changedIds;
    for (let walks = 1; ; walks++) {
      const { updateSet, boundaries } = this.__testWalk(walkFrom);
      // Called for an empty update too, as in Vite: a push that skipped a generation pulls the
      // catch-up.
      const ready = await this.ensureRunnable(updateSet);
      if (ready.type === 'reload') {
        if (this.reloadReason !== undefined) {
          return { status: 'reload', pulls: this.requestNonce - noncesBefore };
        }
        throw new Error(`[test-hmr] full reload: ${ready.reason}`);
      }
      if (ready.type === 'caught-up') {
        if (walks === 2) throw new Error('[test-hmr] full reload: caught up twice');
        walkFrom = [...new Set([...walkFrom, ...ready.changedIds])];
        continue;
      }
      if (updateSet.size === 0) {
        this.appliedGen = Math.max(this.appliedGen, this.knownGen);
        return { status: 'noop', pulls: this.requestNonce - noncesBefore };
      }
      this.__testApply(updateSet, boundaries);
      this.appliedGen = Math.max(this.appliedGen, this.knownGen);
      return { status: 'applied', pulls: this.requestNonce - noncesBefore };
    }
  }

  /**
   * @param {string[]} changedIds
   */
  __testWalk(changedIds) {
    /** @type {[string, string][]} */
    const boundaries = [];
    /** @type {Set<string>} */
    const updateSet = new Set();
    // Vite's `traversedModules`: one set for the whole update, so a module
    // reachable by multiple importer paths is walked (and its boundary recorded) once
    const traversedModules = new Set();
    for (const changed of changedIds) {
      if (!this.isExecuted(changed)) continue;
      const fullReloadReason = this.__testBubble(
        changed,
        [changed],
        updateSet,
        boundaries,
        traversedModules,
      );
      if (fullReloadReason) throw new Error(`[test-hmr] full reload: ${fullReloadReason}`);
    }
    return { updateSet, boundaries };
  }

  /**
   * @param {Set<string>} updateSet
   * @param {[string, string][]} boundaries
   */
  __testApply(updateSet, boundaries) {
    // capture the old generation's callbacks before evictions wipe the contexts
    const applies = boundaries.map(([boundary, acceptedVia]) => ({
      acceptedVia,
      callbacks: (this.contexts.get(boundary)?.callbacks ?? []).filter(({ deps }) =>
        Array.isArray(deps) ? deps.includes(acceptedVia) : deps === acceptedVia,
      ),
    }));
    for (const id of updateSet) this.removeModuleCache(id);
    for (const { acceptedVia, callbacks } of applies) {
      this.initModule(acceptedVia);
      const fresh = this.loadExports(acceptedVia);
      for (const { deps, cb } of callbacks) {
        if (Array.isArray(deps)) {
          cb(deps.map((dep) => (dep === acceptedVia ? fresh : undefined)));
        } else {
          cb(fresh);
        }
      }
    }
  }

  /**
   * @param {string} id
   * @param {string[]} stack
   * @param {Set<string>} updateSet
   * @param {[string, string][]} boundaries
   * @param {Set<string>} traversedModules
   * @returns {string | undefined} full-reload reason
   */
  __testBubble(id, stack, updateSet, boundaries, traversedModules) {
    // cross-path dedup, mirroring Vite's `traversedModules`
    if (traversedModules.has(id)) return;
    traversedModules.add(id);
    updateSet.add(id);
    if (this.isSelfAccepting(id)) {
      boundaries.push([id, id]);
      return;
    }
    const acceptedExports = this.acceptedExportsOf(id);
    if (acceptedExports) {
      boundaries.push([id, id]);
    }
    const parents = this.getImporters(id).filter((p) => this.isExecuted(p));
    if (!parents.length) {
      return acceptedExports ? undefined : `no hmr boundary found for module \`${id}\``;
    }
    for (const parent of parents) {
      if (acceptedExports && parent === id) continue;
      if (this.acceptsDepOf(parent, id)) {
        boundaries.push([parent, id]);
        continue;
      }
      if (acceptedExports) {
        const bindings = this.getImportedBindings(parent, id);
        if (bindings && bindings.every((name) => acceptedExports.has(name))) {
          continue;
        }
      }
      if (stack.includes(parent)) {
        return `circular import chain between \`${id}\` and \`${parent}\``;
      }
      // One shared stack with push/pop instead of a copy per recursion level.
      stack.push(parent);
      const fullReloadReason = this.__testBubble(
        parent,
        stack,
        updateSet,
        boundaries,
        traversedModules,
      );
      stack.pop();
      if (fullReloadReason) return fullReloadReason;
    }
  }
}

/** @type {any} */ (globalThis).__rolldown_runtime__ ??= new TestDevRuntime();
