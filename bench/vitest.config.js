import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["test/**/*.test.js"],
    fileParallelism: false,
    benchmark: {
      include: ["**/*.bench.js"],
      suppressExportGetterWarnings: true,
    },
  },
});
