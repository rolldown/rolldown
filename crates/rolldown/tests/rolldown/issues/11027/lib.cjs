const shared = require('./shared.js');
module.exports = {
  value: 42,
  default: 'own-default',
  ['__proto__']: 'own-proto',
  __toESM: 'own-helper-name',
  shared: shared.value,
};
