export async function f() {
  const missing = "./no-such-file.js";
  return import(missing);
}
