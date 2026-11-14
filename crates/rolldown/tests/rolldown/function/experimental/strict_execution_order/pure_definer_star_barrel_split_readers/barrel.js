// The star re-export is load-bearing: a NAMED re-export of the definer resolves the binding to its
// origin and the consumer calls `init_definer` directly (green). Reaching the pure definer through
// `export *` makes the namespace read depend on the barrel's own `init_*` forwarding to the definer.
export * from './definer.js';
export { vSib } from './sibling.js';
