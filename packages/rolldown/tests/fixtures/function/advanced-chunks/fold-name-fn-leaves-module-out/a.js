import './peer.js';

globalThis.foldNameFnLog.push('a');
export const load = () => import('./lazy.js');
