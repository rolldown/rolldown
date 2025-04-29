import { defineTest } from "rolldown-tests";
import { and, code, exclude, id, not } from "rolldown/filter";
import { expect, vi } from "vitest";

const transformHookFunction = vi.fn(() => {});
export default defineTest({
	config: {
		plugins: [
			{
				name: "filter-expr",
				transform: {
					handler() {
						transformHookFunction();
					},
          filter: [exclude(and(id(/src/), not(code(/import\s+{/))))],
        },
			},
		],
	},
	afterTest: (_) => {
    expect(transformHookFunction).toBeCalledTimes(2);
	},
});
