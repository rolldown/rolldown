(function (factory) {
  if (typeof module === 'object' && typeof module.exports === 'object') {
    let SJS_NS = require('./sheets.cjs');
    factory(SJS_NS);
    module.exports = SJS_NS;
  } else {
    factory(GC);
  }
})(function (GC) {
  console.log(GC.Spread.Sheets.Designer.DR.res);
  GC = GC || {};
  GC.Spread = GC.Spread || {};
  GC.Spread.Sheets = GC.Spread.Sheets || {};
  GC.Spread.Sheets.Designer = GC.Spread.Sheets.Designer || {};
});
