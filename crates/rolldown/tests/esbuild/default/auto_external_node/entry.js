// These URLs should be external automatically
import fs from "node:fs/promises";
fs.readFile();

// This should be external and should be tree-shaken because it's side-effect free
import "node:path";

// This should be external too, but shouldn't be tree-shaken because it could be a run-time error
import "node:what-is-this";