// Simulating a CommonJS module like React
exports.useState = function useState() {
  return ['state', 'setState'];
};

exports.default = function React() {
  return 'React';
};
