module.exports = () =>
  import('./shadowed-arguments.js').then(function (arguments) {
    return arguments.value;
  });
