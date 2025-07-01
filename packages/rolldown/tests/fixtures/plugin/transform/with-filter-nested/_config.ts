import { RolldownPluginOption } from "rolldown";
import {withFilter} from 'rolldown/filter';
import { defineTest } from "rolldown-tests";
import { expect, vi } from "vitest";

const transformFn = vi.fn();
const transformFn1 = vi.fn();
const transformFn2 = vi.fn();

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
					transformFn1();
				},
			},
		},
		{
			name: "test-plugin-1-2",
			transform: {
				handler(_, _id) {
					transformFn2();
				},
			},
		},
	],
];

export default defineTest({
	config: {
		// Without this override, the transform function will be called 9 times
		plugins: [
			withFilter(nestedPlugin, {
				transform: { id: /\.vue$/ },
				pluginNamePattern: [/test-plugin-1-.*/],
			}),
		],
	},
	afterTest: () => {
		expect(transformFn).toHaveBeenCalledTimes(3);
		expect(transformFn1).toHaveBeenCalledTimes(0);
		expect(transformFn2).toHaveBeenCalledTimes(0);
	},
});
