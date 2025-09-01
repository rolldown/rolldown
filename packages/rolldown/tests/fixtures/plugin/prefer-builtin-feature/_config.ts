import { defineTest } from "rolldown-tests";
import { expect } from "vitest";
import { stripAnsi } from "consola/utils";

let warnings: string[] = [];
export default defineTest({
	config: {
		plugins: [
			{
				name: "json",
			},
			{
				name: "inject",
			},
		],
		onwarn(warning, ctx) {
			warnings.push(stripAnsi(warning.message));
		},
	},
	afterTest() {
		expect(warnings).toMatchInlineSnapshot(`
			[
			  "[PREFER_BUILTIN_FEATURE] Warning: The functionality provided by \`@rollup/plugin-json\` is already covered natively, maybe you could remove the plugin from your configuration
			  │ 
			  │ Help: This diagnostic may be false positive, you could turn it off via \`checks.preferBuiltinFeature\`
			",
			  "[PREFER_BUILTIN_FEATURE] Warning: Rolldown supports \`inject\` natively, please refer https://rolldown.rs/reference/config-options for more details, this is performant than passing \`@rollup/plugin-inject\` to plugins option.
			  │ 
			  │ Help: This diagnostic may be false positive, you could turn it off via \`checks.preferBuiltinFeature\`
			",
			]
		`);
	},
});
