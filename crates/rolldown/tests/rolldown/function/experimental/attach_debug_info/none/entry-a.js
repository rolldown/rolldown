import "./shared.js";
import "./share-splitted.js";

console.log("entry-a");
import("./dyn-entry.js").then(console.log);