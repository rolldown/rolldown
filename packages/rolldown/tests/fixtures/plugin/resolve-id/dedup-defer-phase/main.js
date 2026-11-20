import defer * as deferred from './dep.js';
import { value } from './dep.js';

export const load = () => import('./dep.js');
export const loadDeferred = () => import.defer('./dep.js');
export { deferred, value };
