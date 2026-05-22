import assert from "node:assert";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

const dist = fileURLToPath(new URL("dist", import.meta.url));
const files = readdirSync(dist).filter((file) => file.endsWith(".js"));

let broken = false;

for (const file of files) {
  const code = readFileSync(new URL(`dist/${file}`, import.meta.url), "utf8");

  for (const match of code.matchAll(/\b(init_[A-Za-z0-9_$]+)\s*\(/g)) {
    const fn = match[1];
    const hasImport = new RegExp(
      String.raw`\bimport\s*\{[^}]*\b(?:${fn}\b|as\s+${fn}\b)[^}]*\}\s*from\b`,
      "s",
    ).test(code);
    const hasDefinition = new RegExp(String.raw`\b(?:var|let|const|function)\s+${fn}\b`).test(code);

    if (!hasImport && !hasDefinition) {
      broken = true;
      console.error(`${file} calls ${fn} without importing or defining it`);
    }
  }
}

assert.equal(broken, false);

const { get } = await import("./dist/tu.js");
assert.equal(get(), true);
