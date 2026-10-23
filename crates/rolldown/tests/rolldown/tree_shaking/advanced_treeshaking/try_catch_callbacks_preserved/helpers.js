export function guard(name, run) {
  try {
    run();
  } catch {
    globalThis.log.push(name);
  }
}
