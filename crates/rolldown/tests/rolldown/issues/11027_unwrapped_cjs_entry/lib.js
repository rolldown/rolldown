const { shared } = require('./shared.js');

module.exports = { shared, load: () => import('./dynamic.js') };
