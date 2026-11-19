globalThis.log.push('main');
export const loadDeferred = () => import.defer('./other.js');
