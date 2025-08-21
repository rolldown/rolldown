// @ts-nocheck FIXME(hyf0): Enable type check

/// <reference path="../../../crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js" />

class TestHotContext {
  moduleId;
  callbacks = [];

  constructor(moduleId) {
    this.moduleId = moduleId;
  }

  accept(...args) {
    if (args.length === 0) return;
    if (args.length === 1) {
      this.callbacks.push({ deps: this.moduleId, cb: args[0] });
      return;
    }
    this.callbacks.push({ deps: args[0], cb: args[1] });
  }
}

class TestDevRuntime extends DevRuntime {
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
   * @override
   * @param {string[]} boundaries
   */
  applyUpdates(boundaries) {
    for (const moduleId of boundaries) {
      for (const ctx of this.contexts.values()) {
        for (const { deps, cb } of ctx.callbacks) {
          if (Array.isArray(deps)) {
            if (deps.includes(moduleId)) {
              const mods = deps.map((id) =>
                id === moduleId ? this.loadExports(moduleId) : undefined
              );
              cb(mods);
            }
          } else {
            if (deps === moduleId) {
              cb(this.loadExports(moduleId));
            }
          }
        }
      }
    }
  }
}

(/** @type {any} */ (globalThis)).__rolldown_runtime__ ??= new TestDevRuntime();
