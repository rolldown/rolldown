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
   * @param {...any} args
   * @returns {void}
   */
  accept(...args) {
    if (args.length === 0) return;
    if (args.length === 1) {
      this.callbacks.push({ deps: this.moduleId, cb: args[0] });
      return;
    }
    this.callbacks.push({ deps: args[0], cb: args[1] });
  }
}

class TestDevRuntime extends BaseDevRuntime {
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
   * @param {[string, string][]} boundaries
   */
  applyUpdates(boundaries) {
    for (const [boundary, acceptedVia] of boundaries) {
      const ctx = this.contexts.get(boundary);
      if (!ctx) continue;

      for (const { deps, cb } of ctx.callbacks) {
        if (Array.isArray(deps)) {
          if (deps.includes(acceptedVia)) {
            const mods = deps.map((id) =>
              id === acceptedVia ? this.loadExports(acceptedVia) : undefined,
            );
            cb(mods);
          }
        } else {
          if (deps === acceptedVia) {
            cb(this.loadExports(acceptedVia));
          }
        }
      }
    }
  }
}

/** @type {any} */
const messenger = undefined;
const clientId = crypto.randomUUID();

/** @type {any} */ (globalThis).__rolldown_runtime__ ??= new TestDevRuntime(messenger, clientId);
