const y = {
	a: () => () => {},
	[globalThis.unknown]: () => () => console.log('effect')
};

y.a()();

const z = {
	[globalThis.unknown]: () => ({})
};

z.a()();

const v = {};
v.toString().doesNotExist(0); // retained

const w = {
	toString: () => ({
		charCodeAt: () => console.log('effect')
	})
};

w.toString().charCodeAt(0); // retained
