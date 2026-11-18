const shared = require('./shared.cjs');
exports.POST = async () => (await import('./dynamic.cjs')).default === shared;
