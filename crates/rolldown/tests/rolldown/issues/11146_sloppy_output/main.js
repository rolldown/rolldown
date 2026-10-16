import { env } from './dynamic.js';
import * as ns from './nested.js';
import * as server from './server.js';

// Sloppy CJS output ignores these writes instead of throwing `TypeError`, so they stay errors.
env.FOO = 'changed';
delete env.FOO;
ns.sub.FOO = 'changed';
server.FOO = 'changed';
