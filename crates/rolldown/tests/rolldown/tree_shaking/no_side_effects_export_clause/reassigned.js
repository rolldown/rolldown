let reassigned = /* #__NO_SIDE_EFFECTS__ */ (value) => [];

reassigned = (value) => {
  console.log(value);
  return [];
};

export { reassigned };
