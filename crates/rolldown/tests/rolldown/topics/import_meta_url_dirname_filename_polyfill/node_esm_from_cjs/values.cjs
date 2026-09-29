var __dirname;
var __filename;

function getNestedValues() {
  return [__dirname, __filename];
}

function getShadowedValues() {
  const __dirname = 'local dirname';
  const __filename = 'local filename';
  return [__dirname, __filename];
}

function setValues(values) {
  __dirname = values.dirname;
  __filename += values.filename;
  [__dirname, __filename] = values;
  ({ __dirname, __filename } = values);
  ({ dirname: __dirname, filename: __filename } = values);
  __dirname++;
  __filename--;
  return [__dirname, __filename];
}

function callAssignedDirname(fn) {
  __dirname = fn;
  return __dirname();
}

function deleteDirname() {
  return [delete __dirname, typeof __dirname];
}

function deleteParenthesizedDirname() {
  // oxfmt-ignore
  return [delete ((__dirname)), typeof __dirname];
}

module.exports = {
  dirname: __dirname,
  filename: __filename,
  nested: getNestedValues(),
  shorthand: { __dirname, __filename },
  shadowed: getShadowedValues(),
  setValues,
  callAssignedDirname,
  deleteDirname,
  deleteParenthesizedDirname,
};
