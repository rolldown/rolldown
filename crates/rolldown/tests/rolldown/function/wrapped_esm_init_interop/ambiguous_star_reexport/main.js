// The barrel's dynamic exports keep this IsExportStar record included while `x` remains ambiguous
// and is therefore absent from its sorted, non-ambiguous export map. The second entry keeps the
// immediate barrel unwrapped and in a different chunk.
export * from './barrel.js';
globalThis.__log.push('BEFORE_REQUIRE');
// These later requires wrap both owners and make both init symbols reachable in this chunk. The
// star-export collector must not pull either call forward to the re-export position.
require('./mod_a.js');
require('./mod_b.js');
globalThis.__log.push('MAIN');
