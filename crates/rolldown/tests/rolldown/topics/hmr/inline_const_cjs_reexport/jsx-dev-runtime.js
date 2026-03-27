if (process.env.NODE_ENV === 'production') {
  module.exports = require('./jsx-dev-runtime.production.js');
} else {
  module.exports = require('./jsx-dev-runtime.development.js');
}
