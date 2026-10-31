if (globalThis.__loadDynamicExisting) {
  import('./dynamic-existing.js').then((mod) => {
    globalThis.__dynamicExistingValue = mod.value;
  });
}

export const version = 'initial';

import.meta.hot?.accept();
