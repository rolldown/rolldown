import { defineConfig } from "rolldown";
import { ecmaTransformPlugin } from "rolldown/experimental";

export default defineConfig({
	input: "./index.ts",
	resolve: {
		// This needs to be explicitly set for now because oxc resolver doesn't
		// assume default exports conditions. Rolldown will ship with a default that
		// aligns with Vite in the future.
		conditionNames: ["import"],
	},
  plugins: [
    ecmaTransformPlugin(),
    {
      "name": "test",
      transform(code, id,meta) {
        console.log(`code: `, code, meta)
        return null
      }
    }
  ]
});
