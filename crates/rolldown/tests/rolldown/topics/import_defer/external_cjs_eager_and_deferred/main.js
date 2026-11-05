import defer * as ns from 'ext-lazy';
import { eagerValue } from './eager.js';

const log = (globalThis.__extLog ??= []);
log.push('main');
export const read = () => [ns.value, eagerValue()];
