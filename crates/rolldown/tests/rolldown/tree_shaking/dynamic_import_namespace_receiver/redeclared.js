export let value = 'expected';
export var read = function () {
  return 'stale';
};
var read = function () {
  return this.value;
};
