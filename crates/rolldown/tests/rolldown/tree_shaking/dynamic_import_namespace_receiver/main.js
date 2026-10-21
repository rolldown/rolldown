export { value } from './merged.js';
export const merged = () => import('./merged.js').then(ns => ns.read());
export const method = async () => import('./method.js').then(ns => ns.read());
export const computed = async () => import('./computed.js').then(ns => ns['read']());
export const optional = async () => import('./optional.js').then(ns => ns?.read?.());
export const parenthesized = async () => import('./parenthesized.js').then(ns => (ns.read)());
export const tagged = async () => import('./tagged.js').then(ns => ns.read``);
export const awaited = async () => (await import('./awaited.js')).read();
