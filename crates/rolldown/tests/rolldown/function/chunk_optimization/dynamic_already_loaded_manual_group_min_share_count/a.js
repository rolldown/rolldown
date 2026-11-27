import './peer.js';

globalThis.minShareCountLog.push('a');
export const load = () => Promise.all([import('./lazy.js'), import('./other.js')]);
