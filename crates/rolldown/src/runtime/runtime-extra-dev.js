// @ts-check

const hot = {
  accept(...args) {
    if (args.length === 1) {
      const [cb] = args;
      DevRuntime.getInstance().modules['src/Draw.js'].acceptCallbacks.push(cb)
    }
  }
};

class DevRuntime {
  hot = hot
  modules = {}
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
      const { acceptCallbacks } = this.modules[id];
      console.log('Module already registered', id, this.modules[id]);
      acceptCallbacks.forEach((cb) => {

        Promise.resolve().then(() => {
          cb(exports);
        })
      });
      this.modules[id] = {
        exports: exports,
        acceptCallbacks: [],
      }
    } else {
      // If the module is not in the cache, we need to register it.
      this.modules[id] = {
        exports: exports,
        acceptCallbacks: [],
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