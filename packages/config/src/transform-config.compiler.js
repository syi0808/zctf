import {
  ENUMS,
  TRANSFORM_CONFIG,
} from "./layout.generated.js";

const encoder = new TextEncoder();
const MAGIC_CONFIG = 0x4346_435a;
const VERSION = 2;
const HEADER_SIZE = 32;
const ROOT_SIZE = 32;
const LIST_HEADER_SIZE = 16;
const PLUGIN_SIZE = 8;
const STRING_ENTRY_SIZE = 8;
const KNOWN_NAME_BIT = 0x8000_0000;

const PRESENCE = Object.freeze({
  typescript: 1 << 0,
  jsxRuntime: 1 << 1,
  exportType: 1 << 2,
  svgo: 1 << 3,
  plugins: 1 << 4,
  multipass: 1 << 5,
  floatPrecision: 1 << 6,
  svgoPlugins: 1 << 7,
});

const FLAGS = Object.freeze({
  typescript: 1 << 0,
  svgo: 1 << 1,
  multipass: 1 << 2,
});

const PLUGIN_FLAGS = Object.freeze({
  activePresent: 1 << 0,
  active: 1 << 1,
  currentColorPresent: 1 << 2,
  currentColor: 1 << 3,
});

export const KNOWN_CONFIG_NAMES = Object.freeze({
  svgo: 1,
  jsx: 2,
  removeViewBox: 3,
  convertColors: 4,
  currentColor: 5,
});

export class ConfigWriter {
  constructor(capacity) {
    this.bytes = new Uint8Array(capacity);
    this.view = new DataView(this.bytes.buffer);
    this.length = 0;
    this.asciiStrings = 0;
    this.utf8Strings = 0;
    this.knownNames = 0;
  }

  reset(requiredCapacity) {
    if (this.bytes.byteLength < requiredCapacity) {
      this.bytes = new Uint8Array(requiredCapacity);
      this.view = new DataView(this.bytes.buffer);
    } else {
      this.bytes.fill(0, 0, this.length);
    }
    this.length = 0;
    this.asciiStrings = 0;
    this.utf8Strings = 0;
    this.knownNames = 0;
    return this;
  }

  finish(length) {
    this.length = length;
    return this.bytes.subarray(0, length);
  }
}

function enumId(value, values, defaultValue) {
  if (value === undefined || value === defaultValue) return 0;
  const id = values[value];
  if (id === undefined) throw new TypeError(`unsupported enum value: ${value}`);
  return id;
}

function assertPlainObject(value, name) {
  if (value === undefined) return;
  if (value === null || typeof value !== "object") {
    throw new TypeError(`${name} must be a plain object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${name} must be a plain object`);
  }
}

function stringCapacity(value) {
  return value.length * 3;
}

// Exact structural allocation plus a conservative UTF-8 heap upper bound. The
// writer never grows while compiling.
export function estimateTransformConfigSize(config) {
  const plugins = config.plugins ?? [];
  const nestedPlugins = config.svgoConfig?.plugins ?? [];
  let size = HEADER_SIZE + ROOT_SIZE;
  if (plugins.length) size += LIST_HEADER_SIZE + plugins.length * 4;
  if (nestedPlugins.length) size += LIST_HEADER_SIZE + nestedPlugins.length * PLUGIN_SIZE;
  let unknownCount = 0;
  let heapCapacity = 0;
  for (const name of plugins) {
    if (KNOWN_CONFIG_NAMES[name] === undefined) {
      unknownCount++;
      heapCapacity += stringCapacity(name);
    }
  }
  for (const plugin of nestedPlugins) {
    if (KNOWN_CONFIG_NAMES[plugin.name] === undefined) {
      unknownCount++;
      heapCapacity += stringCapacity(plugin.name);
    }
  }
  return size + unknownCount * STRING_ENTRY_SIZE + heapCapacity;
}

function planTransformConfig(config, plugins, nestedPlugins) {
  let structural = HEADER_SIZE + ROOT_SIZE;
  if (plugins.length) structural += LIST_HEADER_SIZE + plugins.length * 4;
  if (nestedPlugins.length) {
    structural += LIST_HEADER_SIZE + nestedPlugins.length * PLUGIN_SIZE;
  }
  let unknownCount = 0;
  let heapCapacity = 0;
  for (const name of plugins) {
    if (KNOWN_CONFIG_NAMES[name] === undefined) {
      unknownCount++;
      heapCapacity += stringCapacity(name);
    }
  }
  for (const plugin of nestedPlugins) {
    if (KNOWN_CONFIG_NAMES[plugin.name] === undefined) {
      unknownCount++;
      heapCapacity += stringCapacity(plugin.name);
    }
  }
  return {
    capacity: structural + unknownCount * STRING_ENTRY_SIZE + heapCapacity,
    tableOffset: structural,
    unknownCount,
  };
}

function writeString(value, bytes, offset, capacity, writer) {
  let index = 0;
  for (; index < value.length; index++) {
    const code = value.charCodeAt(index);
    if (code > 0x7f) break;
    bytes[offset + index] = code;
  }
  if (index === value.length) {
    writer.asciiStrings++;
    return value.length;
  }
  const result = encoder.encodeInto(value, bytes.subarray(offset, offset + capacity));
  if (result.read !== value.length) throw new RangeError("config string heap overflow");
  writer.utf8Strings++;
  return result.written;
}

function nameToken(name, stringIds, writer) {
  const known = KNOWN_CONFIG_NAMES[name];
  if (known !== undefined) {
    writer.knownNames++;
    return (KNOWN_NAME_BIT | known) >>> 0;
  }
  const id = stringIds.length;
  stringIds.push(name);
  return id;
}

function writeListHeader(u32, offset, length, stride) {
  u32(offset, length);
  u32(offset + 4, length);
  u32(offset + 8, stride);
  u32(offset + 12, offset + LIST_HEADER_SIZE);
}

/**
 * Generated, schema-specific fast path. It deliberately performs direct reads
 * from plain JSON-like objects and direct writes to fixed offsets.
 */
export function compileConfigInto(writer, config) {
  assertPlainObject(config, "config");
  const plugins = config.plugins ?? [];
  const svgoConfig = config.svgoConfig;
  assertPlainObject(svgoConfig, "config.svgoConfig");
  const nestedPlugins = svgoConfig?.plugins ?? [];
  for (const plugin of nestedPlugins) assertPlainObject(plugin, "svgo plugin");

  const plan = planTransformConfig(config, plugins, nestedPlugins);
  writer.reset(plan.capacity);
  const { bytes, view } = writer;
  const u32 = (offset, value) => view.setUint32(offset, value, true);

  let cursor = HEADER_SIZE + ROOT_SIZE;
  const pluginsOffset = plugins.length ? cursor : 0;
  if (plugins.length) cursor += LIST_HEADER_SIZE + plugins.length * 4;
  const nestedOffset = nestedPlugins.length ? cursor : 0;
  if (nestedPlugins.length) cursor += LIST_HEADER_SIZE + nestedPlugins.length * PLUGIN_SIZE;

  const tableOffset = cursor;
  cursor += plan.unknownCount * STRING_ENTRY_SIZE;
  const heapOffset = cursor;
  const stringIds = [];

  u32(0, MAGIC_CONFIG);
  u32(4, VERSION);
  u32(8, TRANSFORM_CONFIG.offset);
  u32(12, tableOffset);
  u32(16, heapOffset);
  u32(24, plan.unknownCount);

  let presence = 0;
  let flags = 0;
  if (config.typescript === true) {
    presence |= PRESENCE.typescript;
    flags |= FLAGS.typescript;
  }
  const jsxRuntime = enumId(config.jsxRuntime, ENUMS.jsxRuntime, "automatic");
  if (jsxRuntime) presence |= PRESENCE.jsxRuntime;
  const exportType = enumId(config.exportType, ENUMS.exportType, "default");
  if (exportType) presence |= PRESENCE.exportType;
  if (config.svgo === true) {
    presence |= PRESENCE.svgo;
    flags |= FLAGS.svgo;
  }
  if (plugins.length) presence |= PRESENCE.plugins;
  if (svgoConfig?.multipass === true) {
    presence |= PRESENCE.multipass;
    flags |= FLAGS.multipass;
  }
  const floatPrecision = svgoConfig?.floatPrecision ?? 0;
  if (floatPrecision !== 0) presence |= PRESENCE.floatPrecision;
  if (nestedPlugins.length) presence |= PRESENCE.svgoPlugins;

  const root = TRANSFORM_CONFIG.offset;
  const fields = TRANSFORM_CONFIG.fields;
  u32(root + fields.presence, presence);
  u32(root + fields.flags, flags);
  view.setFloat64(root + fields.floatPrecision, floatPrecision, true);
  u32(root + fields.plugins, pluginsOffset);
  u32(root + fields.svgoPlugins, nestedOffset);
  view.setUint8(root + fields.jsxRuntime, jsxRuntime);
  view.setUint8(root + fields.exportType, exportType);

  if (pluginsOffset) {
    writeListHeader(u32, pluginsOffset, plugins.length, 4);
    for (let index = 0; index < plugins.length; index++) {
      u32(
        pluginsOffset + LIST_HEADER_SIZE + index * 4,
        nameToken(plugins[index], stringIds, writer),
      );
    }
  }

  if (nestedOffset) {
    writeListHeader(u32, nestedOffset, nestedPlugins.length, PLUGIN_SIZE);
    for (let index = 0; index < nestedPlugins.length; index++) {
      const plugin = nestedPlugins[index];
      const item = nestedOffset + LIST_HEADER_SIZE + index * PLUGIN_SIZE;
      let pluginFlags = 0;
      if (plugin.active !== undefined) {
        pluginFlags |= PLUGIN_FLAGS.activePresent;
        if (plugin.active) pluginFlags |= PLUGIN_FLAGS.active;
      }
      if (plugin.currentColor !== undefined) {
        pluginFlags |= PLUGIN_FLAGS.currentColorPresent;
        if (plugin.currentColor) pluginFlags |= PLUGIN_FLAGS.currentColor;
      }
      u32(item, nameToken(plugin.name, stringIds, writer));
      view.setUint16(item + 4, pluginFlags, true);
      // Known params use a fixed plugin kind instead of a generic key/value map.
      view.setUint16(item + 6, KNOWN_CONFIG_NAMES[plugin.name] ?? 0, true);
    }
  }

  let heapCursor = heapOffset;
  for (let id = 0; id < stringIds.length; id++) {
    const value = stringIds[id];
    const capacity = stringCapacity(value);
    const written = writeString(value, bytes, heapCursor, capacity, writer);
    u32(tableOffset + id * STRING_ENTRY_SIZE, heapCursor - heapOffset);
    u32(tableOffset + id * STRING_ENTRY_SIZE + 4, written);
    heapCursor += written;
  }
  u32(20, heapCursor);
  return writer.finish(heapCursor);
}

export function compileConfig(config) {
  return compileConfigInto(new ConfigWriter(0), config);
}

const syncWriter = new ConfigWriter(1024);
let syncWriterInUse = false;

function invokeSync(callback, bytes, writer) {
  const result = callback(bytes, writer);
  if (result === bytes || (result !== null && typeof result?.then === "function")) {
    throw new TypeError("compiled temp buffer cannot escape or cross an async boundary");
  }
  return result;
}

// The returned bytes are valid only during callback execution. This prevents a
// reused temp buffer from escaping across an async boundary.
export function withCompiledConfig(config, callback) {
  if (syncWriterInUse) {
    const local = new ConfigWriter(estimateTransformConfigSize(config));
    return invokeSync(callback, compileConfigInto(local, config), local);
  }
  syncWriterInUse = true;
  try {
    const bytes = compileConfigInto(syncWriter, config);
    return invokeSync(callback, bytes, syncWriter);
  } finally {
    syncWriterInUse = false;
  }
}
