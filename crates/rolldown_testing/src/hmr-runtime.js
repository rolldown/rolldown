// @ts-check

/** @import { DevRuntime } from "../../rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js" */

/** @type {typeof DevRuntime} */
// @ts-expect-error -- there's no way to declare a variable by JSDoc
var BaseDevRuntime = DevRuntime;

class TestHotContext {
  moduleId;
  /** @type {{ deps: string, cb: Function }[]} */
  callbacks = [];

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
    if (args.length === 0 || typeof args[0] === 'function') {
      this.callbacks.push({ deps: this.moduleId, cb: args[0] ?? (() => {}) });
      return;
    }
    this.callbacks.push({ deps: args[0], cb: args[1] ?? (() => {}) });
  }
}

/**
 * Test-harness mirror of the dedicated FBM HMR client: the walk on the runtime's static
 * imports and module cache with acceptance from the live hot contexts, then remove from
 * the module cache + dep-first re-runs from the factory map. Driven per patch by
 * `__testApplyHmr(changedIds)`, the push stand-in. A full-reload update throws —
 * executed-style fixtures assert hot updates, and reload cases must not run under
 * `should_execute_output`.
 */
class TestDevRuntime extends BaseDevRuntime {
  /** @type {Map<string, TestHotContext>} */
  contexts = new Map();

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
   * @param {string[]} changedIds
   */
  __testApplyHmr(changedIds) {
    /** @type {[string, string][]} */
    const boundaries = [];
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
    for (const id of updateSet) {
      if (!this.hasFactory(id)) {
        throw new Error(`[test-hmr] full reload: no factory for ${id}`);
      }
    }
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
    if (this.acceptsDepOf(id, id)) {
      boundaries.push([id, id]);
      return;
    }
    const parents = this.getImporters(id).filter((p) => this.isExecuted(p));
    if (!parents.length) {
      return `no hmr boundary found for module \`${id}\``;
    }
    for (const parent of parents) {
      if (this.acceptsDepOf(parent, id)) {
        boundaries.push([parent, id]);
        continue;
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

const clientId = crypto.randomUUID();

/** @type {any} */ (globalThis).__rolldown_runtime__ ??= new TestDevRuntime(clientId);
