import { unused } from './dep.js';
import * as dep from './dep.js';
import * as barrel from './barrel.js';
import * as nested from './nested.js';

export const values = [dep.default, dep['a-b'], nested.ns.missing];
export const keys = [Object.keys(dep).sort(), Object.keys(barrel).sort()];
export const load = () => import('./barrel.js');
