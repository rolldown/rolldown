(() => {})();
(() => {})(keepThisButRemoveTheIIFE);
(() => { /* @__PURE__ */ removeMe() })();
var someVar;
(x => {})(someVar);
var removeThis = /* @__PURE__ */ (() => stuff())();
var removeThis2 = (() => 123)();