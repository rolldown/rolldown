import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'
import type { LogLevel, RollupLog } from 'rolldown'

const logs: Array<{ level: LogLevel; log: RollupLog }> = []

export default defineTest({
  config: {
    external: /node:path/,
    output: {
      exports: 'named',
      format: 'iife',
    },
    onLog(level, log) {
      logs.push({ level, log })
    },
  },
  afterTest: (output) => {
    expect(logs).toHaveLength(2)
    expect(logs[0].level).toBe('warn')
    expect(logs[0].log.code).toBe('MISSING_NAME_OPTION_FOR_IIFE_EXPORT')
    expect(logs[1].level).toBe('warn')
    expect(logs[1].log.code).toBe('MISSING_GLOBAL_NAME')
    expect(logs[1].log.message).toContain(
      'No name was provided for external module "node:path" in "output.globals" – guessing "node_path".',
    )

    expect(output.output[0].code).toMatchInlineSnapshot(`
      "(function(exports, node_path) {

      "use strict";
      Object.defineProperty(exports, '__esModule', { value: true });
      //#region rolldown:runtime
      var __create = Object.create;
      var __defProp = Object.defineProperty;
      var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
      var __getOwnPropNames = Object.getOwnPropertyNames;
      var __getProtoOf = Object.getPrototypeOf;
      var __hasOwnProp = Object.prototype.hasOwnProperty;
      var __copyProps = (to, from, except, desc) => {
      	if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
      		key = keys[i];
      		if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
      			get: ((k) => from[k]).bind(null, key),
      			enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
      		});
      	}
      	return to;
      };
      var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", {
      	value: mod,
      	enumerable: true
      }) : target, mod));

      node_path = __toESM(node_path);

      //#region main.js
      var main_default = node_path.join;

      exports.default = main_default
      return exports;
      })({}, node_path);"
    `)
  },
})
