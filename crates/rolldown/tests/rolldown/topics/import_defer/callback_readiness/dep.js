try {
  globalThis.read();
} catch (error) {
  globalThis.events.push(error.name);
}
globalThis.events.push(globalThis.readOther());
export const x = 2;
