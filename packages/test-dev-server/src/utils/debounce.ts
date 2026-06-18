/**
 * Collapse a burst of calls into a single trailing invocation, mirroring the
 * `debounce` helper in Vite's `fullBundleEnvironment.ts`.
 */
export function debounce(time: number, cb: () => void): () => void {
  let timer: ReturnType<typeof globalThis.setTimeout> | null = null;
  return () => {
    if (timer) {
      globalThis.clearTimeout(timer);
      timer = null;
    }
    timer = globalThis.setTimeout(cb, time);
  };
}
