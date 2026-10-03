// Heavy CJS stack, reachable ONLY via dynamic import (through heavy.js).
var codec = require('./shared-codec.cjs');
module.exports.sign = function sign(tx) {
  return 'signed:' + codec.decode(tx);
};
module.exports.blob = 'x'.repeat(50000);
