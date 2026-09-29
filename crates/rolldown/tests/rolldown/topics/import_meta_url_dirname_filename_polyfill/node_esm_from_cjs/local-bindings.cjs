function __dirname() {
  return 'local dirname';
}

function __filename() {
  return 'local filename';
}

function deleteLocalDirname() {
  return delete __dirname;
}

module.exports = [__dirname(), __filename(), deleteLocalDirname()];
