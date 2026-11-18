import { env } from './dynamic.js';
import * as ns from './nested.js';

env.FOO = 'changed';
env.FOO++;
[env.FOO] = ['changed'];
for (env.FOO of ['changed']);
delete env.FOO;
delete env?.FOO;
ns.sub.FOO = 'changed';
