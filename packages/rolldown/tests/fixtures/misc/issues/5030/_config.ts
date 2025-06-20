import { defineTest } from "rolldown-tests";
import { expect } from "vitest";

let moduleInfo: any[] = [];
export default defineTest({
	config: {
		plugins: [
			{
				name: "test",
				transform(code, id) {
					if (id.includes("cube")) {
						this.emitFile({
							type: "chunk",
							id,
							preserveSignature: false,
						});
					}
				},
				buildEnd() {
					for (const moduleId of this.getModuleIds()) {
						moduleInfo.push(this.getModuleInfo(moduleId));
					}
				},
			},
		],
	},
	afterTest() {
		expect(moduleInfo).toHaveLength(2);
		for (let i = 0; i < moduleInfo.length; i++) {
			expect(moduleInfo[i].isEntry).toBe(true);
		}
	},
});
