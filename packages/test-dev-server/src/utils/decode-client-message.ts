import { RawData } from 'ws';

import { ClientMessage } from '../types/client-message';

function rawDataToString(data: Buffer | ArrayBuffer | Buffer[]): string {
  if (Buffer.isBuffer(data)) {
    return data.toString('utf8');
  }
  if (Array.isArray(data)) {
    return Buffer.concat(data).toString('utf8');
  }
  return Buffer.from(data).toString('utf8');
}

export function decodeClientMessage(data: RawData): ClientMessage {
  const stringified = rawDataToString(data);
  const decoded = JSON.parse(stringified) as ClientMessage;
  switch (decoded.type) {
    case 'hmr:invalidate':
      return { type: 'hmr:invalidate', moduleId: decoded.moduleId };
    case 'hmr:module-registered':
      return { type: 'hmr:module-registered', modules: decoded.modules };
    default:
      const _never: never = decoded;
      throw new Error(`Unknown client message: ${stringified}`);
  }
}
