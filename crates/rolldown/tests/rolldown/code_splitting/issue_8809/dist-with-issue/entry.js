import { n as start, r as stop } from './daemon.js';
//#region \0rolldown/runtime.js
var __defProp = Object.defineProperty;
var __commonJSMin = (cb, mod) => () => (
  mod || cb((mod = { exports: {} }).exports, mod), mod.exports
);
var __exportAll = (all, no_symbols) => {
  let target = {};
  for (var name in all)
    __defProp(target, name, {
      get: all[name],
      enumerable: true,
    });
  if (!no_symbols) __defProp(target, Symbol.toStringTag, { value: 'Module' });
  return target;
};
//#endregion
//#region gateway.js
var import_cjs_dep = /* @__PURE__ */ __commonJSMin((exports) => {
  exports.helper = function () {
    return 'cjs-helper';
  };
})();
function gateway() {
  return start() + stop();
}
//#endregion
//#region entry.js
function main() {
  return (0, import_cjs_dep.helper)() + gateway();
}
//#endregion
export { main, __exportAll as t };
