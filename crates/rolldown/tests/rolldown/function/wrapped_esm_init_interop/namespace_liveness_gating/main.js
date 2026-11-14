import * as ns from './barrel.js';
// The second entry keeps this forwarder in a different chunk. These later requires wrap both leaf
// owners and make both init symbols reachable here without wrapping the immediate barrel.
globalThis.__log.push('BEFORE_REQUIRE:' + ns.a());
require('./leaf_a.js');
require('./leaf_b.js');
globalThis.__log.push('MAIN');
