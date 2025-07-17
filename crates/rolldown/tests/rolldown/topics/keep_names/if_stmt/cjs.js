if (process.env.NODE_ENV !== 'production') {
  var ReactIs = require('a');

  var a = true;
  module.exports = require('b')(ReactIs.isElement, a);
} else {
  module.exports = require('./c')();
}
