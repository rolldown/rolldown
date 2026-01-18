// Entry point that dynamically imports modules
// The dynamic imports will be captured by manualCodeSplitting,
// resulting in eliminated facade chunks
import { a } from './dynamic-a.js'
import { b } from './dynamic-b.js'
import("./dynamic-a.js").then(console.log);
import("./dynamic-b.js").then(console.log);

console.log(a, b)
