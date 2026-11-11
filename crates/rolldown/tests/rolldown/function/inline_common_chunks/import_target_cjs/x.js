import counter from './counter.cjs';
counter.inc();
globalThis.events.push('X ' + counter.count);
