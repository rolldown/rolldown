import defer * as ns from 'ext';
import './eager.js';
import { both } from './both.js';

export const read = () => ns.value;
export const escape = () => ns;
export const load = () => import.defer('ext2');
export { both };
