import shared from './shared.cjs';

globalThis.firstCommonJsExport = shared;
shared.count++;
globalThis.events.push(`a:${shared.count}`);
