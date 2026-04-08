// Trigger module. The plugin's `transform` hook fires on this file and
// calls `this.emitFile({type:"chunk", ...})` many times. With the
// previously bounded module-loader channel of capacity 1024, this would
// deadlock once enough emits piled up before the loader could drain
// them; now it should complete cleanly regardless of the count.
export const marker = 'trigger';
