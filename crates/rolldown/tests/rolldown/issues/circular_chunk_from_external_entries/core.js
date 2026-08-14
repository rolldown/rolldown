export async function run(code) {
  let pa = await import('./parser-a.js');
  let pb = await import('./parser-b.js');
  let pc = await import('./parser-c.js');
  return pa.parse(code) + pb.parse(code) + pc.parse(code);
}
