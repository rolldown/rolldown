// Pure star-only outer barrel: no local body and no direct execution dependency of its own, so the
// namespace consumers reach the definer's binding through two chained `export *` hops.
export * from './inner-barrel.js';
