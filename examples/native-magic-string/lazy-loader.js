// Example file demonstrating AST + MagicString transformation
// Pattern: fn(() => import(url)) -> fn(() => import(url), url)

function registerLazyModule(loader) {
  console.log('Registering lazy module:', loader);
  return loader();
}

// These calls will be transformed by the plugin
registerLazyModule(() => import('./greet.js'));
registerLazyModule(() => import('./config.js'));
registerLazyModule(() => import('./index.js'));

export { registerLazyModule };
