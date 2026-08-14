// Statically pulls the wrapped `external.js` into the dynamic-import chain.
// The binding itself is unused — only the import edge matters for the repro.
import '../zod/external.js';
