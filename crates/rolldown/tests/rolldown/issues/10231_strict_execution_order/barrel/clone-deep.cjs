module.exports = function cloneDeep(value) {
  return JSON.parse(JSON.stringify(value));
};
