// Non-transparent barrel (it owns a side-effectful execution dependency). Its own star hop to the
// pure definer must forward `init_definer` when a consumer reads the definer's binding through the
// outer barrel's namespace — the delegated remainder of the outer barrel's retained path.
export * from './definer.js';
export { vSib } from './sibling.js';
