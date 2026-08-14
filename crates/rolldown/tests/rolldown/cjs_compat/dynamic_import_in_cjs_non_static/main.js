// Non-static dynamic import with variable
import(moduleName).then(console.log);

// Static import with @vite-ignore (no import record)
import(/* @vite-ignore */ './ignored.js').then(console.log);
