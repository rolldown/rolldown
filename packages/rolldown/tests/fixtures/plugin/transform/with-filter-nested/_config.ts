import { RolldownPluginOption, withFilter } from "rolldown";
import { defineTest } from "rolldown-tests";
import { expect, vi } from "vitest";

const transformFn = vi.fn();

const nestedPlugin: RolldownPluginOption = [
	{
		name: "test-plugin-1",
		transform: {
			handler(_, _id) {
				transformFn();
			},
		},
	},
	[
		{
			name: "test-plugin-1-1",
			transform: {
				handler(_, _id) {
					transformFn();
				},
			},
		},
		{
			name: "test-plugin-1-2",
			transform: {
				handler(_, _id) {
					transformFn();
				},
			},
		},
	],
];

export default defineTest({
	skipComposingJsPlugin: true,
	config: {
    // Without this override, the transform function will be called 9 times
		plugins: [withFilter(nestedPlugin, { transform: { id: /\.vue$/ } })],
	},
	afterTest: () => {
		expect(transformFn).toHaveBeenCalledTimes(0);
	},
});
