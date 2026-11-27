import { s } from './s.js';

globalThis.log.push(`a:${s.value}`);
globalThis.loadFromA = () => import('./d.js');
