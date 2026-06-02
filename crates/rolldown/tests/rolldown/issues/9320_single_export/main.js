async function main() {
  const aNs = await import('./a.js');
  const bNs = await import('./b.js');
  globalThis.__9320_single_export_aNs = aNs;
  globalThis.__9320_single_export_bNs = bNs;
}
globalThis.__9320_single_export_done = main();
