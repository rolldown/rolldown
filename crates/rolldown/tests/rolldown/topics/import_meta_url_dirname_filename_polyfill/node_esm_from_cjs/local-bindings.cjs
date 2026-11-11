function __dirname() {
  return 'local dirname';
}

function __filename() {
  return 'local filename';
}

module.exports = [__dirname(), __filename()];
