import { env } from './dynamic.js';
import * as ns from './nested.js';
import * as server from './server.js';

// The output would add these props to the namespace object instead of throwing `TypeError`.
env.missing = 1;
ns.sub.missing = 1;
server.missing = 1;
server['also missing'] += 1;
