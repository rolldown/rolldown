module.exports = function setPaths(dirname, filename) {
  __dirname = dirname;
  __filename = filename;
  return [__dirname, __filename];
};
