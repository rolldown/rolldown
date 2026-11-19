globalThis.events.push('counter body');
exports.count = 0;
exports.inc = () => ++exports.count;
