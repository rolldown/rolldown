import { createRequire } from "node:module";

async function test() {
  const mod = await import('./dynamic.js');
  console.log(mod.value);
}

test();
