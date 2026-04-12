import { t as __exportAll } from './entry.js';
//#region daemon-impl.js
function start() {
  return 'start';
}
function stop() {
  return 'stop';
}
//#endregion
//#region daemon.js
var daemon_exports = /* @__PURE__ */ __exportAll({
  start: () => start,
  stop: () => stop,
});
//#endregion
export { start as n, stop as r, daemon_exports as t };
