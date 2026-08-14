export async function format(code) {
  let pl = await import('./plugin.js');
  let core = await import('./core.js');
  return core.run(pl.transform(code));
}
