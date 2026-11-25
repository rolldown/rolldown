// @ts-check

/** @import { DevRuntime } from './runtime-extra-dev-common.js' */

/** @type {typeof DevRuntime} */
// @ts-expect-error -- there's no way to declare a variable by JSDoc
var BaseDevRuntime = DevRuntime;

class ModuleHotContext {
  /**
   * @type {{ deps: [string], fn: (moduleExports: Record<string, any>[]) => void }[]}
   */
  acceptCallbacks = [];
  /**
   * @param {string} moduleId
   * @param {InstanceType<BaseDevRuntime>} devRuntime
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

/** @param {string} reason */
function reload(reason) {
  console.log(`[hmr]: Full reload required (${reason}), reloading page`);
  if (typeof location !== 'undefined') {
    location.reload();
  } else {
    console.log('[hmr]: location is undefined, cannot reload page');
  }
}

class DefaultDevRuntime extends BaseDevRuntime {
  /**
   * @type {Map<string, ModuleHotContext>}
   */
  moduleHotContexts = new Map();
  /**
   * @override
   * @param {string} moduleId
   */
  createModuleHotContext(moduleId) {
    const hotContext = new ModuleHotContext(moduleId, this);
    this.moduleHotContexts.set(moduleId, hotContext);
    return hotContext;
  }

  /**
   * @override
   * @param {string} reason
   */
  reloadNeeded(reason) {
    const first = this.reloadReason === undefined;
    super.reloadNeeded(reason);
    if (first) reload(reason);
  }
}

console.debug('HMR runtime loaded', '$ADDR');
// The dev server tells connections apart by this id.
const clientId = crypto.randomUUID();
const addr = new URL('ws://$ADDR');
addr.searchParams.set('clientId', clientId);

const socket = new WebSocket(addr);

/** @type {InstanceType<BaseDevRuntime>} */
const runtime = (/** @type {any} */ (globalThis)).__rolldown_runtime__ ??=
  new DefaultDevRuntime(clientId);

/** @param {MessageEvent} event */
socket.onmessage = function(event) {
  const data = JSON.parse(event.data);
  console.debug('Received message:', data);
  if (data.type === 'connected') {
    // Server acknowledged the connection
    console.debug('[hmr]: Connection established with server');
  } else if (data.type === 'hmr:update') {
    // This runtime has no boundary walk, so it cannot apply an update. It marks the modules
    // stale, so a lazy chunk that arrives late cannot install old factories, and then reloads.
    // A node process cannot reload: its dev server restarts it.
    runtime.markStale(data.staleIds, data.gen);
    if (typeof location !== 'undefined') {
      location.reload();
    } else {
      console.debug('[hmr]: Update received, waiting for a restart');
    }
  } else if (data.type === 'hmr:reload') {
    reload('the server sent a reload');
  }
};
