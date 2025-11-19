new URL(`./foo/${dir}/index.js`, import.meta.url);

new URL(`./foo/${dir}.js`, import.meta.url);

new URL(`./foo/${dir}${file}.js`, import.meta.url);

new URL(`./foo/${dir}${dir2}/index.js`, import.meta.url);

new URL(`${file}.js`, import.meta.url);
