import defer * as deferred from './dep.js';
import { value } from './dep.js';

export const load = () => import.defer('./dep.js');
export { deferred, value };
