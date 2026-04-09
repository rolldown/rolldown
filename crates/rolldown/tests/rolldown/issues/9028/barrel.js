// Barrel module: the re-export of `unused` will be excluded by tree-shaking,
// but under strict execution order the excluded statement still triggers
// `generate_transitive_esm_init` to walk through the import graph.
export { value } from './lib.js';
export { unused } from './cycle-a.js';
