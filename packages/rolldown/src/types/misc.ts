/** @inline */
export type SourcemapPathTransformOption = (
  /** The relative path from the generated `.map` file to the corresponding source file. */
  relativeSourcePath: string,
  /** The fully resolved path of the generated sourcemap file. */
  sourcemapPath: string,
) => string;

/** @inline */
export type SourcemapIgnoreListOption = (
  /** The relative path from the generated `.map` file to the corresponding source file. */
  relativeSourcePath: string,
  /** The fully resolved path of the generated sourcemap file. */
  sourcemapPath: string,
) => boolean;
