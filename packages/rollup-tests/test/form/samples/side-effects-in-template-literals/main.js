let x = 0;

function noEffects () {}

function modifyX () {
	return x++;
}

const a = `${noEffects()}is removed`;

const b = `${globalThis.unknown()}has effects`;

const c = `${modifyX()}has effects`;

const d = noEffects`is removed`;

const e = noEffects`${globalThis.unknown()}has effects`;

const f = noEffects`${modifyX()}has effects`;

const g = globalThis.unknown`has effects`;

const h = (() => {
	console.log( 'effect' );
	return () => {};
})()`has effects`;

const i = modifyX`has effects`;

export { x };
