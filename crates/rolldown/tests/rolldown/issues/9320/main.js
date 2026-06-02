async function main() {
  const formNs = await import('./form.js');
  const actionNs = await import('./action.js');
  globalThis.__9320_formNs = formNs;
  globalThis.__9320_actionNs = actionNs;
}
globalThis.__9320_done = main();
