export interface HmrUpdateMessage {
  type: 'hmr:update';
  url: string;
  path: string;
}

export interface ConnectedMessage {
  type: 'connected';
}
