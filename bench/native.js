import { createRequire } from "node:module";
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const directory = dirname(fileURLToPath(import.meta.url));
const addonName = readdirSync(directory).find((name) => name.endsWith(".node"));
if (!addonName) throw new Error("native addon not found; run `pnpm build` in bench");
const addon = createRequire(import.meta.url)(join(directory, addonName));

export const {
  transformObject,
  transformZctf,
  transformZctfManual,
} = addon;
