// `node_path` is a closure-local initialized with an external `require()`. It must keep its name
// rather than be renamed off the chunk-level `node_path` onto the sibling `node_path$1`.
var node_path = require('node:path');
var node_path$1 = 'sibling';
module.exports = function () {
  return [node_path.sep, node_path$1];
};
