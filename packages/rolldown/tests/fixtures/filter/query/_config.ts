import { defineTest } from "rolldown-tests";
import { include, queries } from "rolldown/filter";
import * as path from "node:path";
import {expect, vi} from 'vitest'

const cb = vi.fn();
const postfixRE = /[?#].*$/;
export function cleanUrl(url: string): string {
	return url.replace(postfixRE, "");
}

export default defineTest({
	config: {
		plugins: [
			{
				name: "test",
				resolveId: {
					filter: [
						include(
							queries({
								test: true,
							}),
						),
					],
					handler(id) {
            cb();
						return path.resolve(import.meta.dirname, cleanUrl(id));
					},
				},
			},
		],
	},
  afterTest() {
    expect(cb).toHaveBeenCalledTimes(1)
  }
});
