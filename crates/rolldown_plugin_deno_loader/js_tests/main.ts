import {
  defineConfig,
  rolldown,
} from "../../../packages/rolldown/dist/esm/index.mjs";
import { denoLoaderPlugin } from "../../../../rolldown/packages/rolldown/dist/esm/experimental-index.mjs";

const configs = [
  defineConfig({
    input: { basic: "./tests/basic/mod.ts" },
    resolve: { conditionNames: ["import"] },
  }),
  defineConfig({
    input: { jsr: "./tests/jsr/mod.ts" },
    resolve: { conditionNames: ["import"] },
    plugins: [
      denoLoaderPlugin({
        importMapString: await fetch(import.meta.resolve("./deno.json"))
          .then(
            (r) => r.text(),
          ),
      }),
    ],
  }),
];

for (const config of configs) {
  await (await rolldown(config)).write(config.output);
}
