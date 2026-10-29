// `unused` is tree-shaken, but leaf remains live through barrel's `used` import. The excluded
// re-export still forwards init_leaf() inside this async wrapper and must await it before continuing.
export { unused } from './leaf.js';

globalThis.__log.push(`CARRIER:${globalThis.__ready}`);
export const marker = 'marker';
