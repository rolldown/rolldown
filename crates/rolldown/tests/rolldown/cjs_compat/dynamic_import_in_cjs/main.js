// External module dynamic import - should be converted to require()
import("external").then(console.log);

// Internal ESM module dynamic import
import("./internal.js").then(console.log);

// Internal CJS module dynamic import
import("./internal-cjs.js").then(console.log);
