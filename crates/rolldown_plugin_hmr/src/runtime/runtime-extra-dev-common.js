// @ts-check
import {
  __export,
  __toCommonJS,
  __toDynamicImportESM,
  __toESM,
  // @ts-expect-error
} from 'rolldown:runtime';

// oxlint-disable-next-line no-unused-vars
export class DevRuntime {
  /**
   * @type {Record<string, { exports: any }>}
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
   * @param {{ exports: any }} module
   */
  registerModule(id, module) {
    this.modules[id] = module;
  }
  /**
   * @param {string} id
   */
  loadExports(id) {
    const module = this.modules[id];
    if (module) {
      return module.exports;
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
}
