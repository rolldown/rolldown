import { describe, test, it, expect, beforeEach, vi } from "vitest";
import { exec, execSync, spawnSync } from "node:child_process";
import exea, { $, execa } from "execa";
import * as path from "path";

import { projectDir, testsDir } from "@tests/utils";

function cliFixturesDir(...joined: string[]) {
	return testsDir("cli/fixtures", ...joined);
}

import { stripAnsi } from "consola/utils";
const binPath = path.resolve(import.meta.dirname, "../../bin/cli.js");
describe("should not hang after running", () => {
	test.skip("basic", async () => {
		const cwd = cliFixturesDir("no-config");
		const _ret = execSync(`node ${binPath}`, { cwd });
	});
});

describe("basic arguments", () => {
	test("should render help message for empty args", async () => {
		const ret = await execa`rolldown`;

		expect(ret.exitCode).toBe(0);
		expect(stripAnsi(ret.stdout)).toMatchSnapshot();
	});
});

describe("cli options for bundling", () => {
	it("should handle single boolean option", async () => {
		const cwd = cliFixturesDir("cli-option-boolean");
		const status = await $({ cwd })`node ${binPath} --minify -d dist`;
		expect(status.exitCode).toBe(0);
		expect(stripAnsi(status.stdout)).toMatchSnapshot();
	});

	it("should handle single boolean short options", async () => {
		const cwd = cliFixturesDir("cli-option-short-boolean");
		const status = await $({ cwd })`node ${binPath} index.ts -m -d dist`;
		expect(status.exitCode).toBe(0);
		expect(stripAnsi(status.stdout)).toMatchSnapshot();
	});

	it("should handle single string options", async () => {
		const cwd = cliFixturesDir("cli-option-string");
		const status = await $({
			cwd,
		})`node ${binPath} index.ts --format cjs -d dist`;
		expect(status.exitCode).toBe(0);
		expect(stripAnsi(status.stdout)).toMatchSnapshot();
	});

	it("should handle single array options", async () => {
		const cwd = cliFixturesDir("cli-option-array");
		const status = await $({
			cwd,
		})`node ${binPath} index.ts --external node:path --external node:url -d dist`;
		expect(status.exitCode).toBe(0);
		expect(stripAnsi(status.stdout)).toMatchSnapshot();
	});

	it("should handle single object options", async () => {
		const cwd = cliFixturesDir("cli-option-object");
		const status = await $({
			cwd,
		})`node ${binPath} index.ts --module-types .123=text --module-types notjson=json --module-types .b64=base64 -d dist`;
		expect(status.exitCode).toBe(0);
		expect(stripAnsi(status.stdout)).toMatchSnapshot();
	});

	it("should handle negative boolean options", async () => {
		const cwd = cliFixturesDir("cli-option-no-external-live-bindings");
		const status = await $({
			cwd,
		})`rolldown index.ts --format iife --external node:fs --no-external-live-bindingsfjei`;
		expect(status.exitCode).toBe(0);
		expect(stripAnsi(status.stdout)).toMatchSnapshot();
	});
});

describe("config", () => {
	it("should bundle in ext-js-syntax-cjs", async () => {
		const cwd = cliFixturesDir("ext-js-syntax-cjs");
		const status = await $({ cwd })`rolldown -c rolldown.config.js`;
		expect(status.exitCode).toBe(0);
	});
	it("should not bundle in ext-js-syntax-esm", async () => {
		const cwd = cliFixturesDir("ext-js-syntax-esm");
		try {
			const _ = await $({ cwd })`rolldown -c rolldown.config.js`;
		} catch (err) {
			expect(err).not.toBeUndefined();
		}
	});
});
