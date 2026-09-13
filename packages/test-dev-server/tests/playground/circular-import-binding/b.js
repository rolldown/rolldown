// Back-edge that forms the import cycle a.js -> b.js -> a.js.
import { defaults } from './a.js';

export class B {}

// `defaults` is used only inside a function body, so its binding resolves
// fine — only the module-init use of `B` over in a.js breaks.
export const usesA = () => defaults;
