import {
  defineConfig,
  rolldown,
} from "../../../packages/rolldown/dist/esm/index.mjs";
import { denoLoaderPlugin } from "../../../../rolldown/packages/rolldown/dist/esm/experimental-index.mjs";

const configs = [
  defineConfig({
    input: { basic: "./tests/basic/mod.ts" },
    resolve: { conditionNames: ["import"] },
    output: { minify: false },
  }),
  defineConfig({
    input: { jsr: "./tests/jsr/mod.ts" },
    resolve: { conditionNames: ["import"] },
    output: { minify: false },
    plugins: [
      denoLoaderPlugin({
        entryPoints: ["./tests/jsr/mod.ts"],
        importMapBaseUrl: import.meta.resolve("./"),
        importMap: await fetch(import.meta.resolve("./deno.json"))
          .then(
            (r) => r.text(),
          ),
      }),
    ],
  }),
];

for (const config of configs.slice(1)) {
  console.time("rolldown+deno_loader_plugin");
  await (await rolldown(config)).write(config.output);
  console.timeEnd("rolldown+deno_loader_plugin");
}
