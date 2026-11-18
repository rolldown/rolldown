const set = () => {
  const exports = 'nested';
  const module = 'nested-module';
  this.viaArrow = [exports, module];
};
set();
