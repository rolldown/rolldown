export async function run() {
  const myLib = await import("./lib.js");
  return myLib.value;
}
