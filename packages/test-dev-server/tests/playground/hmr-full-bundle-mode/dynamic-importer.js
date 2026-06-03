if (globalThis.__loadDynamicExisting) {
  import('./dynamic-existing.js').then((mod) => {
    globalThis.__dynamicExistingValue = mod.value;
  });
}

export const version = 'initial';

if (import.meta.hot) {
  import.meta.hot.accept();
}
