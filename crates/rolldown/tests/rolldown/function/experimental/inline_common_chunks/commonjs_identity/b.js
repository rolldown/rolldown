import shared from './shared.cjs';

globalThis.events.push(`same:${shared === globalThis.firstCommonJsExport}`);
shared.count++;
globalThis.events.push(`b:${shared.count}`);
