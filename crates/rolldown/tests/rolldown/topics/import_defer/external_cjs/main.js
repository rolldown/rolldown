import defer * as ns from 'ext-lazy';

const log = (globalThis.__extLog ??= []);
log.push('main');
export const escape = () => ns;
export const hasValue = () => 'value' in ns;
export const read = () => ns.value;
export const load = () => import.defer('ext-lazy');
