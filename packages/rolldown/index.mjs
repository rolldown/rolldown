import madge from "madge";
import { resolve, join } from "path";
import { globSync } from "glob";

const baseDir = resolve(import.meta.dirname, "./src");

const allFile = globSync(`${baseDir}/**/*.ts`);
// console.log(`allFile: `, allFile);
let set = new Set();
madge(resolve(import.meta.dirname, "./src/index.ts")).then((res) => {
	Object.keys(res.obj()).forEach((item) => {
		const p = join(baseDir, item);
    console.log(`p: `, p)
    set.add(p)
	});
});

await madge(resolve(import.meta.dirname, "./src/cli/index.ts")).then((res) => {
	Object.keys(res.obj()).forEach((item) => {
		const p = join(baseDir, item);
    console.log(`p: `, p)
    set.add(p)
	});
});

await madge(resolve(import.meta.dirname, "./src/parallel-plugin.ts")).then((res) => {
	Object.keys(res.obj()).forEach((item) => {
		const p = join(baseDir, item);
    console.log(`p: `, p)
    set.add(p)
	});
});

await madge(resolve(import.meta.dirname, "./src/experimental-index.ts")).then((res) => {
	Object.keys(res.obj()).forEach((item) => {
		const p = join(baseDir, item);
    console.log(`p: `, p)
    set.add(p)
	});
});

for (let f of allFile) {
  if (!set.has(f)) {
    console.log('unused', f)
  }
}
