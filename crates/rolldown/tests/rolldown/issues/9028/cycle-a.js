// cycle-a and cycle-b form a circular dependency.
// Both are not included (unused), so generate_transitive_esm_init
// will keep recursing through their import records without terminating
// if there is no visited guard.
export { unused } from './cycle-b.js';
