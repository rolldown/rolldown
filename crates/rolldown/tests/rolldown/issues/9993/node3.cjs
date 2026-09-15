// CJS module that is also entry-2. It requires an ESM module (node5.js) that is
// NOT captured by the chunk group, which pulls the runtime helper out of the
// group chunk and into the sibling entry chunk.
globalThis.__issue_9993_3 = (globalThis.__issue_9993_3 || 0) + 1;
void require('./node5.js');
exports.node_3 = 3;
