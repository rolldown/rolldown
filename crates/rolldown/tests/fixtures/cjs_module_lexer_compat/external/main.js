function foo() {
  if (Math.random() > 0.5) {
    module.exports = require('ext')
    } else {
    module.exports = require('./ext.js')
  }
}
console.log(foo)
