import { defineTest } from "rolldown-tests";
import { expect } from "vitest";

let normalizedOutputOptions: object;
export default defineTest({
	config: {
		output: {
      preserveModules: true,
      preserveModulesRoot: "src",
      virtualDirname: "virtual",
		},

		plugins: [
			{
				name: "options",
				renderStart(opts) {
          const descriptors = Object.getOwnPropertyDescriptors(opts);
					normalizedOutputOptions = Object.keys(descriptors).reduce((acc, key: any) => {
						acc[key] = (opts as any)[key];
						return acc;
					}, Object.create(null));
				},
			},
		],
	},
	afterTest: () => {
		expect(normalizedOutputOptions).toMatchInlineSnapshot(`
			{
			  "inner": BindingNormalizedOptions {},
			  "normalizedOutputPlugins": [],
			  "outputOptions": {
			    "preserveModules": true,
			    "preserveModulesRoot": "src",
			    "virtualDirname": "virtual",
			  },
			}
		`);
	},
});
