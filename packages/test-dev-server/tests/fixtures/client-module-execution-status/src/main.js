function load(id) {
  if (id === 'parent-executed') {
    return import('./parent-executed.js');
  }
  if (id === 'parent-non-executed') {
    return import('./parent-non-executed.js');
  }
  throw new Error(`Unknown module: ${id}`);
}

load('parent-executed');

globalThis.records = [];
