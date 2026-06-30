const COUNTS = Object.freeze({
  small: 0,
  medium: 10,
  large: 100,
  stringHeavy: 100,
  pluginHeavy: 100,
  enumHeavy: 0,
  unicodeHeavy: 100,
  defaultHeavy: 0,
});

const KNOWN_NAMES = ["svgo", "jsx", "removeViewBox", "convertColors", "currentColor"];

export const CONFIG_SHAPES = Object.freeze(Object.keys(COUNTS));

export function createConfigFixture(shape = "medium") {
  const count = COUNTS[shape];
  if (count === undefined) throw new TypeError(`unknown config fixture: ${shape}`);
  const suffix =
    shape === "stringHeavy"
      ? "-a-very-long-plugin-name-for-string-heavy-input"
      : shape === "unicodeHeavy"
        ? "-플러그인-설정"
        : "";
  return {
    typescript: shape !== "defaultHeavy",
    jsxRuntime: shape === "enumHeavy" ? "classic" : "automatic",
    exportType: shape === "enumHeavy" ? "named" : "default",
    svgo: shape !== "defaultHeavy",
    plugins: count
      ? Array.from({ length: count }, (_, index) =>
          shape === "pluginHeavy" ? KNOWN_NAMES[index % KNOWN_NAMES.length] : `plugin-${index}${suffix}`,
        )
      : undefined,
    svgoConfig: count
      ? {
          multipass: true,
          floatPrecision: 3,
          plugins: Array.from({ length: count }, (_, index) => ({
            name:
              shape === "pluginHeavy"
                ? KNOWN_NAMES[index % KNOWN_NAMES.length]
                : `svgo-plugin-${index}${suffix}`,
            active: index % 3 !== 0,
            currentColor: index % 2 === 0,
          })),
        }
      : undefined,
  };
}
