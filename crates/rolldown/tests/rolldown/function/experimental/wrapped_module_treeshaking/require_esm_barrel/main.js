import nodeAssert from 'node:assert';

// `require(esm)` wraps `consumer.mjs` (WrapKind::Esm) WITHOUT strictExecutionOrder,
// and recursively the barrel it named-imports from — exercising wrappedModuleTreeshaking
// on a non-strict wrap trigger.
const consumer = require('./consumer.mjs');

nodeAssert.equal(consumer.result, 'used-value');
