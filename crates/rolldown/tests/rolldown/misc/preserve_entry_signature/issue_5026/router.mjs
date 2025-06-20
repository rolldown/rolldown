export const createRouter = (page) => {
	let readyResolve;
	const isReady = new Promise((_r) => {
		readyResolve = _r;
	});
	page().then(() => readyResolve());
	return {
		isReady,
	};
};

export const foo = "foo";
