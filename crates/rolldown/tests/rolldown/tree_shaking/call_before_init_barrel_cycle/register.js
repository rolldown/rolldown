export function register(name) {
  (globalThis.icons ??= []).push(name);
  return name;
}
