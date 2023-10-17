System.register([], (function (exports) {
  'use strict';
  return {
    execute: (function () {

      exports('x', x);

      var _missingExportShim = void 0;

      function x (arg) {
        sideEffect(arg);
      }

      exports({
        default: _missingExportShim,
        missingExport: _missingExportShim,
        missingFn: _missingExportShim
      });

    })
  };
}));
