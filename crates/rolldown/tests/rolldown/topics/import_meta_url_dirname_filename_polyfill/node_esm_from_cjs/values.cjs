function getNestedValues() {
  return [__dirname, __filename];
}

function getShadowedValues() {
  const __dirname = 'local dirname';
  const __filename = 'local filename';
  return [__dirname, __filename];
}

module.exports = {
  nested: getNestedValues(),
  shadowed: getShadowedValues(),
};
