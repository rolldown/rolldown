// A shared module that reads the globals `exports` and `share_shared`, the names the carrier
// would otherwise give the factory parameter and the bridge.
export const kind = typeof exports + '/' + typeof share_shared;
export let n = 1;
