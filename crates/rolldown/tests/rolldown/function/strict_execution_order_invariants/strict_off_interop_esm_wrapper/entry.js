const ns = require('./esm.js');

globalThis.__events = globalThis.__events || [];
globalThis.__events.push('entry:' + ns.value);
