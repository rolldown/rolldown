export enum Status {
  Ok = 1,
}

export function check(Status: { Ok: string }) {
  return Status.Ok;
}
