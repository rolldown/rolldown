export interface HmrInvalidateMessage {
  type: 'hmr:invalidate';
  moduleId: string;
}

type ClientMessage = HmrInvalidateMessage;

export function decodeClientMessageFrom(message: string): ClientMessage {
  const data = JSON.parse(message);
  if (data.type === 'hmr:invalidate') {
    return { type: 'hmr:invalidate', moduleId: data.moduleId };
  }

  throw new Error(`Unknown client message: ${message}`);
}
