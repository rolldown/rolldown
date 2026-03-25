(function (factory) {
  if (typeof module === 'object' && typeof module.exports === 'object') {
    module.exports = factory(require('./sheets.cjs'));
  } else {
    factory(GC);
  }
})(function (GC) {
  GC = GC || {};
  GC.Spread = GC.Spread || {};
  GC.Spread.Sheets = GC.Spread.Sheets || {};
  GC.Spread.Sheets.Designer = GC.Spread.Sheets.Designer || {};
  GC.Spread.Sheets.Designer.DR = GC.Spread.Sheets.Designer.DR || {};
  return {};
});
