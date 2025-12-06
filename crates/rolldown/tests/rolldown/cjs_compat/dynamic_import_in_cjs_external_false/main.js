export async function run() {
  const myLib = await import("some-external-lib");
  return myLib.value;
}
