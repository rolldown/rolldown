import { read } from './shared.js';

function shadow() {}
globalThis.retained.push(shadow);
globalThis.values.push(`a:${read()}:${typeof __rd_share}:${typeof __rd_share_require}`);
