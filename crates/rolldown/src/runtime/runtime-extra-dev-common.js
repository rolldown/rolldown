// @ts-check

// oxlint-disable-next-line no-unused-vars
class DevRuntime {
  /**
   * @type {Record<string, { exports: any }>}
   */
  modules = {}
  /**
   * @param {string} _moduleId
   */
  createModuleHotContext(_moduleId) {
    throw new Error('createModuleHotContext should be implemented')
  }
  /**
   *
   * @param {string[]} _boundaries
   */
  applyUpdates(_boundaries) {
    throw new Error('applyUpdates should be implemented')
  }
  /**
   * @param {string} id
   * @param {{ exports: any }} module
   */
  registerModule(id, module) {
    console.debug('Registering module', id, module);
    this.modules[id] = module
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
  createEsmInitializer = (fn, res) => () => (fn && (res = fn(fn = 0)), res)
  /**
   * __commonJSMin
   *
   * @type {<T extends { exports: any }>(cb: any, mod: { exports: any }) => () => T}
   * @internal
   */
  createCjsInitializer = (cb, mod) => () => (mod || cb((mod = { exports: {} }).exports, mod), mod.exports)
  /** @internal */
  // @ts-expect-error it exists
  __toESM = __toESM;
  /** @internal */
  // @ts-expect-error it exists
  __toCommonJS = __toCommonJS
  /** @internal */
  // @ts-expect-error it exists
  __export = __export
}
