import { value } from './shared.js';

globalThis.foldManualChunksLog.push(`main:${value}`);
export const load = () => import('./dynamic.js');
