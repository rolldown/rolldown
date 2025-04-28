export type SourcemapPathTransformOption = (
  relativeSourcePath: string,
  sourcemapPath: string,
) => string;

export type SourcemapIgnoreListOption = (
  relativeSourcePath: string,
  sourcemapPath: string,
) => boolean;
