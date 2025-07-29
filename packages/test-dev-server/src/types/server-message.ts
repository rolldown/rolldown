export interface UpdateMessage {
  // (hyf0) TODO: This should be `hmr:update`
  type: 'update';
  url: string;
  path: string;
}
