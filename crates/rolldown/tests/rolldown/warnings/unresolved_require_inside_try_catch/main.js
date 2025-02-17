try {
  require("./suppressed.js");
  import('./unresolved1');
} catch {}

try {
  function test() {
    require('./unresolved2')
  }
  class T {
    constructor() {
      require('./unresolved3')
    }
    a = require('./b.js')
  }
  T();
  const a = {
    a: require('./suppressed1.js'),
    b() {
      require('./unresolved4')
    }
  }
} catch {

}
