const mod = require("./lib");

module.exports = {
  // [Object object]
  meta: "rule".length + String(mod).length,
  create: function () {
    console.log("loaded rule");
  },
};
