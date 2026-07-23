const m = require('./main.js');
if (m.ok !== true) {
  throw new Error(`entry exports not visible: ${JSON.stringify(m)}`);
}
module.exports = { ok: true };
