// @ts-check
import {
  __export,
  __reExport,
  __toCommonJS,
  __toDynamicImportESM,
  __toESM,
  // @ts-expect-error
} from 'rolldown:runtime';

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

// oxlint-disable-next-line no-unused-vars
export class DevRuntime {
  /**
   * @param {WebSocket} socket
   */
  constructor(socket) {
    this.socket = socket;
  }

  /**
   * @type {Record<string, Module>}
   */
  modules = {};
  /**
   * @param {string} _moduleId
   */
  createModuleHotContext(_moduleId) {
    throw new Error('createModuleHotContext should be implemented');
  }
  /**
   * @param {string[]} _boundaries
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

  /**
   * __esmMin
   *
   * @type {<T>(fn: any, res: T) => () => T}
   * @internal
   */
  createEsmInitializer = (fn, res) => () => (fn && (res = fn(fn = 0)), res);
  /**
   * __commonJSMin
   *
   * @type {<T extends { exports: any }>(cb: any, mod: { exports: any }) => () => T}
   * @internal
   */
  createCjsInitializer =
    (cb, mod) =>
    () => (mod || cb((mod = { exports: {} }).exports, mod), mod.exports);
  /** @internal */
  __toESM = __toESM;
  /** @internal */
  __toCommonJS = __toCommonJS;
  /** @internal */
  __export = __export;
  /** @internal */
  __toDynamicImportESM = __toDynamicImportESM;
  /** @internal */
  __reExport = __reExport;

  sendModuleRegisteredMessage = (() => {
    const cache = /** @type {string[]} */ ([]);
    let timeout = /** @type {NodeJS.Timeout | null} */ (null);
    let timeoutSetLength = 0;
    const self = this;

    /**
     * @param {string} module
     */
    return function sendModuleRegisteredMessage(module) {
      if (!self.socket) {
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

            if (self.socket.readyState === WebSocket.OPEN) {
              self.socket.send(JSON.stringify({
                type: 'hmr:module-registered',
                modules: cache,
              }));
              cache.length = 0;
            } else if (self.socket.readyState === WebSocket.CLOSED) {
              // Do nothing
            } else {
              self.socket.onopen = function() {
                flushCache();
              };
            }

            timeout = null;
            timeoutSetLength = 0;
          },
        );
        timeoutSetLength = cache.length;
      }
    };
  })();
}
