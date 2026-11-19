// Sloppy code: Annex B block functions inside a linked catch write the enclosing `var`.
var log = [];

// Module-level linked catch in a CJS-wrapped module (the root scope is the closure body).
try {
  throw 0;
} catch (x) {
  var x = 1;
  {
    function x() {
      return 'fx';
    }
    log.push(x());
  }
  log.push(typeof x);
}
log.push(typeof x);

// A fixed name (the lowered `import()` below prints `Promise`), linked at the root of a CJS-wrapped
// module.
try {
  throw 0;
} catch (Promise) {
  var Promise = 'p';
  {
    function Promise() {
      return 'fp';
    }
    log.push(Promise());
  }
  log.push(typeof Promise);
}
log.push(typeof Promise);
const lazy = () => import('./other.js').then((other) => other.x);

// A linked name that is also a fixed name of CJS output.
function fixed() {
  const out = [];
  try {
    throw 0;
  } catch (require) {
    var require = 1;
    {
      function require() {
        return 2;
      }
      out.push(require());
    }
    out.push(typeof require);
  }
  out.push(typeof require);
  return out;
}

function fixedModuleLevel() {
  const out = [];
  try {
    throw 0;
  } catch (__dirname) {
    var __dirname = 'local';
    out.push(__dirname);
  }
  out.push(__dirname);
  return out;
}

function nestedAnnexB() {
  const out = [];
  try {
    throw 'outer';
  } catch (y) {
    try {
      throw 'inner';
    } catch (y) {
      var y = 'set';
      {
        function y() {}
      }
      out.push(y);
    }
    out.push(y);
  }
  out.push(typeof y);
  return out;
}

// Inner linked catches named like the entry's top-level ones.
function inner() {
  try {
    throw 0;
  } catch (y) {
    var y = 'iy';
  }
  try {
    throw 0;
  } catch (z) {
    var z = 'iz';
  }
  return [y, z];
}

const esm = require('./esm.js');

module.exports = { log, fixed, fixedModuleLevel, nestedAnnexB, inner, lazy, esm };
