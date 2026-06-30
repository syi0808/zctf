import { createRequire } from "node:module";
import { existsSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

const require = createRequire(import.meta.url);
const nativeDir = resolve(import.meta.dirname, "../../../native");
const preferred = resolve(nativeDir, `zctf-napi.${process.platform}-${process.arch}.node`);
const candidate = existsSync(preferred)
  ? preferred
  : resolve(nativeDir, readdirSync(nativeDir).find((name) => name.endsWith(".node")) ?? "");

export default require(candidate);

