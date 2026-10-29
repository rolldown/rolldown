globalThis.__events.push('a:start');
require('./bridge.js');
globalThis.__events.push('a:end');

module.exports = 'a';
