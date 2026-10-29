exports.filename = module.filename;
exports.hasParent = module.parent !== null;
exports.isMain = require.main === module;

console.log(
  JSON.stringify({
    filename: module.filename,
    parentIsNull: module.parent === null,
    isMain: require.main === module,
  }),
);
