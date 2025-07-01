import { withFilter } from "rolldown/filter";
import { defineTest } from "rolldown-tests";
import { expect, vi } from "vitest";

const transformFn = vi.fn();
const transformFn2 = vi.fn();
const transformFn3 = vi.fn();

export default defineTest({
	config: {
		plugins: [
			withFilter(
				{
					name: "test-plugin",
					// should override the original filter
					transform: {
						filter: {
							id: /\.[j|t]s$/,
						},
						handler(_, _id) {
							transformFn();
						},
					},
				},
				{
					transform: {
						id: /\.vue$/,
					},
				},
			),
			withFilter(
				{
					name: "test-plugin2",
					// should add filter to transform function
					transform() {
						transformFn2();
					},
				},
				{
					transform: {
						id: /\.vue$/,
					},
				},
			),
			withFilter(
				{
					name: "test-plugin3",
					// should have no effects
					transform() {
						transformFn3();
					},
				},
				{
					resolveId: {
						id: /\.vue$/,
					},
				},
			),
		],
	},
	afterTest: () => {
		expect(transformFn).toHaveBeenCalledTimes(0);
		expect(transformFn2).toHaveBeenCalledTimes(0);
		expect(transformFn3).toHaveBeenCalledTimes(3);
	},
});
