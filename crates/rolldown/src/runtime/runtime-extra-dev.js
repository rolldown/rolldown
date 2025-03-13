// @ts-check

class ModuleHotContext {
  /**
   * @type {{ deps: [string], fn: (moduleExports: Record<string, any>[]) => void }[]}
   */
  acceptCallbacks = []
  /**
   * 
   * @param {string} moduleId 
   * @param {DevRuntime} devRuntime 
   */
  constructor(moduleId, devRuntime) {
    this.moduleId = moduleId;
    this.devRuntime = devRuntime;
  }

  accept(...args) {
    if (args.length === 1) {
      const [cb] = args;
      const acceptingPath = this.moduleId;
      this.acceptCallbacks.push({
        deps: [acceptingPath],
        fn: cb,
      })
    } else {
      throw new Error('Invalid arguments for `import.meta.hot.accept`');
    }
  }
}

class DevRuntime {
  /**
   * @type {Map<string, Set<(...args: any[]) => void>>}
   */
  acceptPathToCallers = new Map()
  modules = {}
  /**
   * @type {Map<string, ModuleHotContext>}
   */
  moduleHotContexts = new Map()
  /**
   * @type {Map<string, ModuleHotContext>}
   */
  moduleHotContextsToBeUpdated = new Map()
  /**
   * 
   * @returns {DevRuntime}
   */
  static getInstance() {
    /**
     * @type {DevRuntime | undefined}
     */
    let instance = globalThis.__rolldown_runtime__;
    if (!instance) {
      instance = new DevRuntime();
      globalThis.__rolldown_runtime__ = instance;
    }
    return instance
  }
  createModuleHotContext(moduleId) {
    const hotContext = new ModuleHotContext(moduleId, this);
    if (this.moduleHotContexts.has(moduleId)) {
      this.moduleHotContextsToBeUpdated.set(moduleId, hotContext);
    } else {
      this.moduleHotContexts.set(moduleId, hotContext);
    }
    return hotContext;
  }
  /**
   * 
   * @param {string[]} boundaries 
   */
  applyUpdates(boundaries) {
    // trigger callbacks of accept() correctly
    for (let moduleId of boundaries) {
      const hotContext = this.moduleHotContexts.get(moduleId);
      if (hotContext) {
        const acceptCallbacks = hotContext.acceptCallbacks;
        acceptCallbacks.filter((cb) => {
          cb.fn(this.modules[moduleId].exports);
        })
      }
    }
    this.moduleHotContextsToBeUpdated.forEach((hotContext, moduleId) => {
      this.moduleHotContexts[moduleId] = hotContext;
    })
    this.moduleHotContextsToBeUpdated.clear()
    // swap new contexts
  }
  registerModule(id, exportGetters) {
    const exports = {};
    Object.keys(exportGetters).forEach((key) => {
      if (Object.prototype.hasOwnProperty.call(exportGetters, key)) {
        Object.defineProperty(exports, key, {
          enumerable: true,
          get: exportGetters[key],
        });
      }
    })
    console.debug('Registering module', id, exports);
    if (this.modules[id]) {
      this.modules[id] = {
        exports,
      }
    } else {
      // If the module is not in the cache, we need to register it.
      this.modules[id] = {
        exports,
      };
    }
  }

  loadExports(id) {
    const module = this.modules[id];
    if (module) {
      return module.exports;
    } else {
      console.warn(`Module ${id} not found`);
      return {};
    }
  }

  // __esmMin
  createEsmInitializer = (fn, res) => () => (fn && (res = fn(fn = 0)), res)
  // __commonJSMin
  createCjsInitializer = (cb, mod) => () => (mod || cb((mod = { exports: {} }).exports, mod), mod.exports)
}

globalThis.__rolldown_runtime__ = DevRuntime.getInstance();

function loadScript(url) {
  var script = document.createElement('script');
  script.src = url;
  script.type = 'module';
  script.onerror = function () {
    console.error('Failed to load script: ' + url);
  }
  document.body.appendChild(script);
}


const socket = new WebSocket(`ws://localhost:3000`)

socket.onmessage = function (event) {
  const data = JSON.parse(event.data)
  if (data.type === 'update') {
    loadScript(data.url)
    console.debug('Module updated');
  }
}