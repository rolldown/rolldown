undef = (() => {})();
(() => { keepMe() })();
((x = keepMe()) => {})();
var someVar;
(([y]) => {})(someVar);
(({z}) => {})(someVar);