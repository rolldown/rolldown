const TypeId = '~effect/DEAD_TypeId';
const Proto = {
  [TypeId]: TypeId,
  pipe() {
    return 1;
  },
  toJSON() {
    return 2;
  },
};
({ ...Proto });
({ ...Proto });
export const done = true;
