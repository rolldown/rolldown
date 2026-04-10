//#region completion.js
async function complete() {
  return (await import('./daemon.js').then((n) => n.t)).start();
}
//#endregion
export { complete };
