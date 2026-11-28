const log = (globalThis.__extDynLog ??= []);
log.push('main');
export const loadCjs = () => import.defer('ext-cjs');
export const loadTla = () => import.defer('ext-tla');
