globalThis.__events.push('first');
// Requiring `second.js` wraps it, so `main.js` initializes it with `init_second()`.
module.exports = { value: 17, second: require('./second.js') };
