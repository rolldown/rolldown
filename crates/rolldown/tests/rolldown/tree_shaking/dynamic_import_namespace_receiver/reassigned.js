export let value = 'expected';
export let read = function () {
  return 'stale';
};
read = function () {
  return this.value;
};
