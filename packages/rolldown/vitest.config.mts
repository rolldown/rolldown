import { defineConfig } from "vitest/config";
import nodePath from "node:path";

export default defineConfig({
	test: {
    pool: 'forks',
    poolOptions: {
      forks: {
        'singleFork': true
      }
    },
		testTimeout: 20000,
		// Disabled, Because the error printed by rust cannot be seen
		disableConsoleIntercept: true,
		// https://vitest.dev/api/mock.html#mockreset, since we run each test twice, so we need to reset the mockReset for each run
		mockReset: true,
		// onConsoleLog(log: string, type: "stdout" | "stderr"): boolean | void {
		// 	return !(log === "message from third party library" && type === "stdout");
		// },
	},
	resolve: {
		alias: {
			"@tests": nodePath.resolve(__dirname, "tests/src"),
			"@src": nodePath.resolve(__dirname, "src"),
		},
	},
	esbuild: {
		target: "node18",
	},
});
