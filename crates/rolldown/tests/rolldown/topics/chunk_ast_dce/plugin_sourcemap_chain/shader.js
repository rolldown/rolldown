export default `
  #ifdef USE_MAP
    vec4 texel = texture2D( map, uv );
  #endif
`;
