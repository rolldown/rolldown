// @ts-check

class ModuleHotContext {
  /**
   * @type {{ deps: [string], fn: (moduleExports: Record<string, any>[]) => void }[]}
   */
  acceptCallbacks = [];
  /**
   * @param {string} moduleId
   * @param {DevRuntime} devRuntime
   */
  constructor(moduleId, devRuntime) {
    this.moduleId = moduleId;
    this.devRuntime = devRuntime;
  }

  /**
   * @overload
   * @param {(mod: Record<string, any>) => void} cb
   * @returns {void}
   */
  /**
   * @param {...any} args
   * @returns {void}
   */
  accept(...args) {
    if (args.length === 1) {
      const [cb] = /** @type {[(mod: Record<string, any>) => void]} */ (args);
      const acceptingPath = this.moduleId;
      this.acceptCallbacks.push({
        deps: [acceptingPath],
        fn: cb,
      });
    } else if (args.length === 0) {}
    else {
      throw new Error('Invalid arguments for `import.meta.hot.accept`');
    }
  }

  invalidate() {
    socket.send(JSON.stringify({
      type: 'hmr:invalidate',
      moduleId: this.moduleId,
    }));
  }
}

class DefaultDevRuntime extends DevRuntime {
  /**
   * @type {Map<string, ModuleHotContext>}
   */
  moduleHotContexts = new Map();
  /**
   * @type {Map<string, ModuleHotContext>}
   */
  moduleHotContextsToBeUpdated = new Map();
  /**
   * @override
   * @param {string} moduleId
   */
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
   * @override
   * @param {[string, string][]} boundaries
   */
  applyUpdates(boundaries) {
    // trigger callbacks of accept() correctly
    for (let [moduleId, acceptedVia] of boundaries) {
      const hotContext = this.moduleHotContexts.get(moduleId);
      if (hotContext) {
        const acceptCallbacks = hotContext.acceptCallbacks;
        acceptCallbacks.filter((cb) => {
          cb.fn(this.modules[moduleId].exports);
        });
      }
    }
    this.moduleHotContextsToBeUpdated.forEach((hotContext, moduleId) => {
      this.moduleHotContexts.set(moduleId, hotContext);
    });
    this.moduleHotContextsToBeUpdated.clear();
    // swap new contexts
  }
}

(/** @type {any} */ (globalThis)).__rolldown_runtime__ ??=
  new DefaultDevRuntime();

/** @param {string} url */
function loadScript(url) {
  var script = document.createElement('script');
  script.src = url;
  script.type = 'module';
  script.onerror = function() {
    console.error('Failed to load script: ' + url);
  };
  document.body.appendChild(script);
}

console.debug('HMR runtime loaded', '$ADDR');
const addr = new URL('ws://$ADDR');

const socket = new WebSocket(addr);

/** @param {MessageEvent} event */
socket.onmessage = function(event) {
  const data = JSON.parse(event.data);
  console.debug('Received message:', data);
  if (data.type === 'hmr:update') {
    if (typeof process === 'object') {
      import(data.path);
      console.debug(`[hmr]: Importing HMR patch: ${data.path}`);
    } else {
      console.debug(`[hmr]: Loading HMR patch: ${data.path}`);
      loadScript(data.url);
    }
  }
};
