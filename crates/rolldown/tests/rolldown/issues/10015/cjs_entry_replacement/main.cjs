const shared = require('./shared.cjs');
module.exports = { POST: async () => (await import('./dynamic.cjs')).default === shared };
