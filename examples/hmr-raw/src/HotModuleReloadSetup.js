class HotModuleReloadSetup {
  constructor() {
    this.modules = {};
    this.instances = {};
    this.constructorArgs = {};

    document.body.addEventListener('hot-module-reload', (event) => {
      const { newModule } = event.detail;
      this.swapModule(newModule);
    });
  }

  swapModule(newModule) {
    const name = newModule.default.name;
    console.debug('Swapping module', name);
    const oldModule = this.modules[name];
    const oldInstance = this.instances[name];
    if (!oldModule) return;

    const newInstance = new newModule.default(...this.constructorArgs[name]);
    newInstance.hotReload(oldInstance);

    this.modules[name] = newModule;
    this.instances[name] = newInstance;
  }

  import(newModule, ...args) {
    const newInstance = new newModule.default(...args);

    const name = newModule.default.name;
    console.debug('Importing module', name);
    this.modules[name] = newModule;
    this.instances[name] = newInstance;
    this.constructorArgs[name] = args;
  }
}

export default HotModuleReloadSetup;

export function HMREventHandler(newModule) {
  console.log('HMR event handler called', newModule);
  const event = new CustomEvent('hot-module-reload', { detail: { newModule } });
  document.body.dispatchEvent(event);
}
