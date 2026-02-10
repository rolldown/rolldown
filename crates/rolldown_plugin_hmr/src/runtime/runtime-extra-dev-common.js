// @ts-check
import {
  __exportAll,
  __reExport,
  __toCommonJS,
  __toESM,
  // @ts-expect-error
} from '\0rolldown/runtime.js';

class Module {
  /**
   * @param {string} id
   */
  constructor(id) {
    /** @type {{ exports: any }} */
    this.exportsHolder = { exports: null };
    /** @type {string} */
    this.id = id;
  }

  get exports() {
    return this.exportsHolder.exports;
  }
}

/**
 * @typedef {{ type: 'hmr:module-registered', modules: string[] }} DevRuntimeMessage
 * @typedef {{ send(message: DevRuntimeMessage): void }} Messenger
 */

export class DevRuntime {
  /**
   * @param {Messenger} messenger
   * @param {string} clientId
   */
  constructor(messenger, clientId) {
    this.messenger = messenger;
    /** @type {string} */
    this.clientId = clientId;
    /** @type {Record<string, Module>} */
    this.modules = {};

    /**
     * __esmMin
     * @type {<T>(fn: any, res: T) => () => T}
     * @internal
     */
    this.createEsmInitializer = (fn, res) => () => (fn && (res = fn((fn = 0))), res);
    /**
     * __commonJSMin
     * @type {<T extends { exports: any }>(cb: any, mod: { exports: any }) => () => T}
     * @internal
     */
    this.createCjsInitializer = (cb, mod) => () => (
      mod || cb((mod = { exports: {} }).exports, mod), mod.exports
    );
    /** @internal */
    this.__toESM = __toESM;
    /** @internal */
    this.__toCommonJS = __toCommonJS;
    /** @internal */
    this.__exportAll = __exportAll;
    /**
     * @param {boolean} [isNodeMode]
     * @returns {(mod: any) => any}
     * @internal
     */
    this.__toDynamicImportESM = (isNodeMode) => (mod) => __toESM(mod.default, isNodeMode);
    /** @internal */
    this.__reExport = __reExport;

    this.sendModuleRegisteredMessage = (() => {
      const cache = /** @type {string[]} */ ([]);
      let timeout = /** @type {NodeJS.Timeout | null} */ (null);
      let timeoutSetLength = 0;
      const self = this;

      /**
       * @param {string} module
       */
      return function sendModuleRegisteredMessage(module) {
        if (!self.messenger) {
          return;
        }
        cache.push(module);
        if (!timeout) {
          timeout = setTimeout(
            /** @returns void */
            function flushCache() {
              if (cache.length > timeoutSetLength) {
                timeout = setTimeout(flushCache);
                timeoutSetLength = cache.length;
                return;
              }

              self.messenger.send({
                type: 'hmr:module-registered',
                modules: cache,
              });
              cache.length = 0;
              timeout = null;
              timeoutSetLength = 0;
            },
          );
          timeoutSetLength = cache.length;
        }
      };
    })();
  }

  /**
   * @param {string} _moduleId
   */
  createModuleHotContext(_moduleId) {
    throw new Error('createModuleHotContext should be implemented');
  }
  /**
   * @param {[string, string][]} _boundaries
   */
  applyUpdates(_boundaries) {
    throw new Error('applyUpdates should be implemented');
  }
  /**
   * @param {string} id
   * @param {{ exports: any }} exportsHolder
   */
  registerModule(id, exportsHolder) {
    const module = new Module(id);
    module.exportsHolder = exportsHolder;
    this.modules[id] = module;
    this.sendModuleRegisteredMessage(id);
  }
  /**
   * @param {string} id
   */
  loadExports(id) {
    const module = this.modules[id];
    if (module) {
      return module.exportsHolder.exports;
    } else {
      console.warn(`Module ${id} not found`);
      return {};
    }
  }
}
