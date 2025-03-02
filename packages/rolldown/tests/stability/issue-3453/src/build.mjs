import { build } from "rolldown";
import { parseImportMap, resolveModuleSpecifier } from "./import-maps.js";
import * as fs from "fs";
import json from "./map.json" with { type: "json" };
import * as path from "path";
import {isWindows} from './util.js'

if (isWindows()) {
  // TODO: enable this test on Windows
  process.exit(0);
}

const __dirname = path.dirname(new URL(import.meta.url).pathname);

const importMapJSON =
	'{"imports":{"__framer-not-found-page":"https://framer.com/m/framer/SitesNotFoundPage.js@1.1.0","#framer/local/collection/pxB40Bw2G/pxB40Bw2G.js":"https://framerusercontent.dev/modules/LG0loF7IY1Mdj6ONzAhW/BmSIcMsm657jwKUt8t0W/pxB40Bw2G.js","#framer/local/collection/U5J_P2oWm/U5J_P2oWm.js":"https://framerusercontent.dev/modules/yclpWreWbUYg71AtgF0p/U88ywOxGdYeFqXAjpfMC/U5J_P2oWm.js","#framer/local/css/AevQplO1L/AevQplO1L.js":"https://framerusercontent.dev/modules/TvKwmdf0JZOsUOMGYQ96/y5JHshNhi4GJ6fdzCHNb/AevQplO1L.js","#framer/local/css/FLLlKeALY/FLLlKeALY.js":"https://framerusercontent.dev/modules/JjgwHWTUJskQ8YRiDsgk/VFT8vNBrdTpiPmPqsRVN/FLLlKeALY.js","#framer/local/css/gDa8VqCZz/gDa8VqCZz.js":"https://framerusercontent.dev/modules/rE6BNXXppuBeRS85NyeZ/1oput0mcca5hgNE4T7FA/gDa8VqCZz.js","#framer/local/css/kccnzi2pd/kccnzi2pd.js":"https://framerusercontent.dev/modules/Z3kuZjb9BGFK4gTKPcl5/we7efoqWXpzP4yXXMQ8M/kccnzi2pd.js","#framer/local/screen/DeL0q0H0v/DeL0q0H0v.js":"https://framerusercontent.dev/modules/cUmP60oZsBS7gKsHEyLv/4z3ICMttLBn1hxcyCoio/DeL0q0H0v.js","#framer/local/screen/Gkwjsv2el/Gkwjsv2el.js":"https://framerusercontent.dev/modules/zLFB8zCF8UV1x0ywwZvx/e3aGBuhO3d2mJhmbfnvK/Gkwjsv2el.js","#framer/local/screen/v2piBMke6/v2piBMke6.js":"https://framerusercontent.dev/modules/sJVl05lpwHddjmPmrB3B/lNmQ2oXzskCps1syixEY/v2piBMke6.js","#framer/local/screen/xOoeiNxXJ/xOoeiNxXJ.js":"https://framerusercontent.dev/modules/E754SxKhDhOsCwkeFkVd/zaN2rwE0nygiUpmXLsYa/xOoeiNxXJ.js","#framer/local/screen/ZhEOYjjNc/ZhEOYjjNc.js":"https://framerusercontent.dev/modules/vR8f3gt9CUH2ZO4XBSiE/cDL2NskBfjDgqPcPfE7Q/ZhEOYjjNc.js","#framer/local/siteMetadata/siteMetadata/siteMetadata.js":"https://framerusercontent.dev/modules/PN4DUl1LUW0KAUYYLv3u/ixQZB2DunvNaCrnSfSwo/siteMetadata.js","#framer/local/webPageMetadata/DeL0q0H0v/DeL0q0H0v.js":"https://framerusercontent.dev/modules/1emnvRaZYB1uupYCZJ7y/mTP6tTVNA2URUKTP0b2h/DeL0q0H0v.js","#framer/local/webPageMetadata/Gkwjsv2el/Gkwjsv2el.js":"https://framerusercontent.dev/modules/5xFhAgbub0EVsn99TJSm/hx56ooyaGOxpH1xz023o/Gkwjsv2el.js","#framer/local/webPageMetadata/v2piBMke6/v2piBMke6.js":"https://framerusercontent.dev/modules/375KR2KvdVnUN8GVmJ7c/DulrBmd8FSKbwAwkwE4C/v2piBMke6.js","#framer/local/webPageMetadata/xOoeiNxXJ/xOoeiNxXJ.js":"https://framerusercontent.dev/modules/bVp3yyLGNiuRptPkD0cn/O7tpIlmpFWJtlG06xj34/xOoeiNxXJ.js","#framer/local/webPageMetadata/ZhEOYjjNc/ZhEOYjjNc.js":"https://framerusercontent.dev/modules/DqRxuMfGuzrhoA0gZrSh/BYyamNnog1OiHCQmLlm4/ZhEOYjjNc.js","framer":"https://app.framerstatic.com/framer.3KX2OI4Q.mjs","framer-motion":"https://app.framerstatic.com/framer-motion.KB2VX5JL.mjs","react":"https://ga.jspm.io/npm:react@18.2.0/index.js","react-dom":"https://ga.jspm.io/npm:react-dom@18.2.0/index.js","react-dom/client":"https://ga.jspm.io/npm:react-dom@18.2.0/client.js","react-dom/server":"https://ga.jspm.io/npm:react-dom@18.2.0/server.browser.js","react/jsx-runtime":"https://ga.jspm.io/npm:react@18.2.0/jsx-runtime.js"},"scopes":{"https://ga.jspm.io/":{"fs":"https://framer.com/m/framer/empty.js@0.1.0","process":"https://framer.com/m/framer/empty.js@0.1.0","scheduler":"https://ga.jspm.io/npm:scheduler@0.23.0/index.js"},"https://esm.sh/":{"fs":"https://framer.com/m/framer/empty.js@0.1.0","process":"https://framer.com/m/framer/empty.js@0.1.0","scheduler":"https://ga.jspm.io/npm:scheduler@0.23.0/index.js"}}}';
function rolldownImportMapsPlugin() {
	const importMap = parseImportMap(importMapJSON);
	return {
		name: "importMaps",
		resolveId(source, importer) {
			if (source.endsWith("main-script.js")) return;

			let baseURL = null;
			if (importer) {
				try {
					baseURL = new URL(importer);
				} catch {
					// Let baseURL stay null.
				}
			}
			const resolvedURL = resolveModuleSpecifier(importMap, source, baseURL);
			return resolvedURL.toString();
		},
		async load(id) {
			if (id.endsWith("main-script.js")) return;
			let number = json[id];
			let filePath = path.resolve(
				__dirname,
				"lib",
				`file${number}.js`,
			);
			let file = fs.readFileSync(filePath, "utf8");
			return file;
		},
	};
}
for (let i = 0; i < 10; i++) {
	build({
		input: path.join(__dirname, "main-script.js"),
		cwd: path.resolve(__dirname, "../"),
		output: {
			dir: `dist${i}`,
			entryFileNames: "[name].[hash].js",
		},
		plugins: [rolldownImportMapsPlugin()],
	});
}
