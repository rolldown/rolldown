import { s } from './s.js';

globalThis.log.push(`a:${s()}`);
globalThis.loadFromA = () => import('./d.js');
