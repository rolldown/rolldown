(() => {})();
(() => {})(keepThisButRemoveTheIIFE);
(() => { /* @__PURE__ */ removeMe() })();
var someVar;
(x => {})(someVar);