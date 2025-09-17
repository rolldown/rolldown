function load(id) {
  if (id === 'parent1') {
    return import('./parent1.js');
  }
  if (id === 'parent2') {
    return import('./parent2.js');
  }
  throw new Error(`Unknown module: ${id}`);
}

load('parent1');

globalThis.records = [];
