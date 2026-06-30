// The pre-optimization v1 compiler is retained only as a benchmark control.
import { MAGIC_CONFIG } from "../../runtime/src/memory.js";

const encoder = new TextEncoder();

function enumId(value, values) {
  if (value === undefined) return 0;
  const id = values.indexOf(value) + 1;
  if (id === 0) throw new TypeError(`unsupported enum value: ${value}`);
  return id;
}

export function compileConfigBaseline(config) {
  const plugins = config.plugins ?? [];
  const svgoConfig = config.svgoConfig;
  const nestedPlugins = svgoConfig?.plugins ?? [];
  const strings = [...plugins, ...nestedPlugins.map((plugin) => plugin.name)];
  const encoded = strings.map((value) => encoder.encode(value));
  let cursor = 56;
  const svgoOffset = svgoConfig ? cursor : 0;
  if (svgoConfig) cursor += 24;
  const pluginsOffset = plugins.length ? cursor : 0;
  if (plugins.length) cursor += 16 + plugins.length * 4;
  const nestedOffset = nestedPlugins.length ? cursor : 0;
  if (nestedPlugins.length) cursor += 16 + nestedPlugins.length * 16;
  const tableOffset = cursor;
  cursor += strings.length * 8;
  const heapOffset = cursor;
  const totalLength = heapOffset + encoded.reduce((sum, value) => sum + value.length, 0);
  const bytes = new Uint8Array(totalLength);
  const view = new DataView(bytes.buffer);
  const u32 = (offset, value) => view.setUint32(offset, value, true);
  const u64 = (offset, value) => view.setBigUint64(offset, BigInt(value), true);
  u32(0, MAGIC_CONFIG);
  u32(4, 1);
  u32(8, 32);
  u32(12, tableOffset);
  u32(16, heapOffset);
  u32(20, totalLength);
  u32(24, strings.length);
  let presence = 0n;
  const fields = ["typescript", "jsxRuntime", "exportType", "svgo", "plugins", "svgoConfig"];
  fields.forEach((field, index) => {
    if (config[field] !== undefined) presence |= 1n << BigInt(index);
  });
  u64(32, presence);
  view.setUint8(40, config.typescript ? 1 : 0);
  view.setUint8(41, enumId(config.jsxRuntime, ["automatic", "classic"]));
  view.setUint8(42, enumId(config.exportType, ["default", "named"]));
  view.setUint8(43, config.svgo ? 1 : 0);
  u32(44, pluginsOffset);
  u32(48, svgoOffset);
  if (pluginsOffset) {
    u32(pluginsOffset, plugins.length);
    u32(pluginsOffset + 4, plugins.length);
    u32(pluginsOffset + 8, 4);
    u32(pluginsOffset + 12, pluginsOffset + 16);
    plugins.forEach((_, index) => u32(pluginsOffset + 16 + index * 4, index));
  }
  if (svgoOffset) {
    let nestedPresence = 0n;
    if (svgoConfig.multipass !== undefined) nestedPresence |= 1n;
    if (svgoConfig.floatPrecision !== undefined) nestedPresence |= 2n;
    if (svgoConfig.plugins !== undefined) nestedPresence |= 4n;
    u64(svgoOffset, nestedPresence);
    view.setUint8(svgoOffset + 8, svgoConfig.multipass ? 1 : 0);
    u32(svgoOffset + 12, nestedOffset);
    view.setFloat64(svgoOffset + 16, svgoConfig.floatPrecision ?? 0, true);
  }
  if (nestedOffset) {
    u32(nestedOffset, nestedPlugins.length);
    u32(nestedOffset + 4, nestedPlugins.length);
    u32(nestedOffset + 8, 16);
    u32(nestedOffset + 12, nestedOffset + 16);
    nestedPlugins.forEach((plugin, index) => {
      const item = nestedOffset + 16 + index * 16;
      let itemPresence = 1n;
      if (plugin.active !== undefined) itemPresence |= 2n;
      if (plugin.currentColor !== undefined) itemPresence |= 4n;
      u64(item, itemPresence);
      u32(item + 8, plugins.length + index);
      view.setUint8(item + 12, plugin.active ? 1 : 0);
      view.setUint8(item + 13, plugin.currentColor ? 1 : 0);
    });
  }
  let heapCursor = heapOffset;
  encoded.forEach((value, id) => {
    u32(tableOffset + id * 8, heapCursor - heapOffset);
    u32(tableOffset + id * 8 + 4, value.length);
    bytes.set(value, heapCursor);
    heapCursor += value.length;
  });
  return bytes;
}
