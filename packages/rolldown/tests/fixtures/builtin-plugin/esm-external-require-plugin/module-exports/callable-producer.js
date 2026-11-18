function callable(value) {
  return value + 1;
}

callable.kind = 'commonjs';

export default 'not the CommonJS export';
export { callable as 'module.exports' };
