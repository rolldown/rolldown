import { defineTest } from "rolldown-tests";
import { expect, vi } from "vitest";

const transformFn = vi.fn();

export default defineTest({
	config: {
		plugins: [
			{
				name: "test-plugin",
				transform: {
					filter: {code: 'import.meta'},
					handler: function () {
						transformFn();
					},
				},
			},
		],
	},
	afterTest: () => {
		expect(transformFn).toHaveBeenCalledTimes(1);
	},
});
