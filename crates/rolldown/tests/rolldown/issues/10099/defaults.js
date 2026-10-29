// Mirrors element-plus/es/defaults.mjs. Under `moduleSideEffects: false` this
// `defaults_default` definition is prunable, so tree-shaking may drop it -- and
// then must also drop every statement that reads `defaults_default`.
import { makeInstaller } from './make-installer.js';

var defaults_default = makeInstaller([]);

export { defaults_default as default };
