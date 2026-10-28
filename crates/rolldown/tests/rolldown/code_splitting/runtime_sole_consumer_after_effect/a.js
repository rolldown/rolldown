import './patch.js';
import legacy from './legacy.cjs';
Object.defineProperty = globalThis.originalDefineProperty;
export const value = legacy.value;
