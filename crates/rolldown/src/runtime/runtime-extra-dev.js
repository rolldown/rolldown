const __modules = globalThis.__modules || (globalThis.__modules = {});

const hot = {
    accept(...args) {
      if (args.length === 1) {
        const [cb] = args;
        __modules['src/Draw.js'].acceptCallbacks.push(cb)
      }
    }
};

/**
 * This object contains the runtime helpers for the development environment.
 */
export const __runtime = {
  registerModule(id, exports) {
    const realExports = {};
    Object.keys(exports).forEach((key) => {
      if (Object.prototype.hasOwnProperty.call(exports, key)) {
        Object.defineProperty(realExports, key, {
          enumerable: true,
          get() {
            return exports[key]();
          },
        });
      }
    })
    console.debug('Registering module', id, realExports);
    if (__modules[id]) {
      const { acceptCallbacks } = __modules[id];
      console.log('Module already registered', id, __modules[id]);
      acceptCallbacks.forEach((cb) => {

        Promise.resolve().then(() => {
          cb(realExports);
        })
      });
      __modules[id] = {
        exports: realExports,
        acceptCallbacks: [],
      }
    } else {
      // If the module is not in the cache, we need to register it.
      __modules[id] = {
        exports: realExports,
        acceptCallbacks: [],
      };
    }
  },
  loadExports(moduleId) {
    const module = __modules[moduleId];
    if (module) {
      return module.exports;
    } else {
      console.warn(`Module ${moduleId} not found`);
      return {};
    }
  },
  hot,
  modules: __modules,
}

globalThis.__rolldown_runtime__ = __runtime;



Object.assign(import.meta, {
  hot,
})

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