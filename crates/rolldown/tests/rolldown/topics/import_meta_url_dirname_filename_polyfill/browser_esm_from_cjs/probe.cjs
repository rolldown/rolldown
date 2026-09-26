function readLocals(__dirname, __filename) {
  return [__dirname, __filename];
}

module.exports = {
  ambient: [typeof __dirname, typeof __filename],
  locals: readLocals('local dirname', 'local filename'),
};
