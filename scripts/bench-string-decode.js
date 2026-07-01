import { performance } from "node:perf_hooks";
import { BenchReportView } from "../packages/bench/fixtures/bench-report.view.js";
import native from "../packages/bench/src/native.js";

const utf8Decoder = new TextDecoder("utf-8");
const latin1Decoder = new TextDecoder("latin1");
const warmupRounds = 3;
const measuredRounds = 9;
const targetBytesPerRound = 16 * 1024 * 1024;
const minimumIterations = 20_000;
const maximumIterations = 2_000_000;
let sink = 0;

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

function measure(run) {
  for (let round = 0; round < warmupRounds; round++) sink ^= run();
  const samples = [];
  for (let round = 0; round < measuredRounds; round++) {
    global.gc?.();
    const startedAt = performance.now();
    sink ^= run();
    samples.push(performance.now() - startedAt);
  }
  return median(samples);
}

function benchmark({ name, encoding, decoder, payload }) {
  const copies = 256;
  const stride = payload.length + 7;
  const bytes = Buffer.allocUnsafe(stride * copies);
  for (let index = 0; index < copies; index++) {
    payload.copy(bytes, index * stride);
  }
  const iterations = Math.max(
    minimumIterations,
    Math.min(maximumIterations, Math.ceil(targetBytesPerRound / payload.length)),
  );
  const mask = copies - 1;

  const bufferMilliseconds = measure(() => {
    let checksum = 0;
    for (let index = 0; index < iterations; index++) {
      const start = (index & mask) * stride;
      const value = bytes.toString(encoding, start, start + payload.length);
      checksum ^= value.charCodeAt(index % value.length);
    }
    return checksum;
  });

  const decoderMilliseconds = measure(() => {
    let checksum = 0;
    for (let index = 0; index < iterations; index++) {
      const start = (index & mask) * stride;
      const value = decoder.decode(bytes.subarray(start, start + payload.length));
      checksum ^= value.charCodeAt(index % value.length);
    }
    return checksum;
  });

  const bufferNs = (bufferMilliseconds * 1e6) / iterations;
  const decoderNs = (decoderMilliseconds * 1e6) / iterations;
  return {
    case: name,
    bytes: payload.length,
    iterations,
    "Buffer ns/op": Number(bufferNs.toFixed(1)),
    "TextDecoder ns/op": Number(decoderNs.toFixed(1)),
    "Buffer/TextDecoder": Number((decoderNs / bufferNs).toFixed(2)),
  };
}

function repeatedUtf8(text, minimumBytes) {
  return Buffer.from(text.repeat(Math.ceil(minimumBytes / Buffer.byteLength(text))));
}

function codePoints(value) {
  return [...value].map((character) =>
    `U+${character.codePointAt(0).toString(16).toUpperCase().padStart(4, "0")}`,
  );
}

const utf8Samples = [
  Buffer.from("package-1234"),
  Buffer.from("한글🙂café"),
  Buffer.from([0x61, 0x80, 0x62]),
  Buffer.from([0xe2, 0x82]),
];
const latin1Sample = Buffer.from([
  0x41, 0x7f, 0x80, 0x81, 0x82, 0x8d, 0x91, 0x9f, 0xa0, 0xff,
]);

console.log(`Node ${process.version} / ${process.platform}-${process.arch}`);
console.log("\nCorrectness: UTF-8");
console.table(
  utf8Samples.map((bytes) => {
    const buffer = bytes.toString("utf8");
    const textDecoder = utf8Decoder.decode(bytes);
    return {
      hex: bytes.toString("hex"),
      equal: buffer === textDecoder,
      buffer: JSON.stringify(buffer),
      textDecoder: JSON.stringify(textDecoder),
    };
  }),
);

const bufferLatin1 = latin1Sample.toString("latin1");
const decoderLatin1 = latin1Decoder.decode(latin1Sample);
console.log(`\nCorrectness: latin1 (TextDecoder normalized label: ${latin1Decoder.encoding})`);
console.table([
  {
    input: latin1Sample.toString("hex"),
    equal: bufferLatin1 === decoderLatin1,
    "Buffer code points": codePoints(bufferLatin1).join(" "),
    "TextDecoder code points": codePoints(decoderLatin1).join(" "),
  },
]);

console.log("\nPerformance (median of 9 rounds; ratio > 1 means Buffer is faster)");
const cases = [
  {
    name: "utf8/ascii-short",
    encoding: "utf8",
    decoder: utf8Decoder,
    payload: Buffer.from("package-1234"),
  },
  {
    name: "utf8/non-ascii-short",
    encoding: "utf8",
    decoder: utf8Decoder,
    payload: Buffer.from("한글🙂café"),
  },
  {
    name: "utf8/ascii-256B",
    encoding: "utf8",
    decoder: utf8Decoder,
    payload: repeatedUtf8("package-1234", 256),
  },
  {
    name: "utf8/non-ascii-256B",
    encoding: "utf8",
    decoder: utf8Decoder,
    payload: repeatedUtf8("한글🙂café", 256),
  },
  {
    name: "utf8/ascii-4KiB",
    encoding: "utf8",
    decoder: utf8Decoder,
    payload: repeatedUtf8("package-1234", 4096),
  },
  {
    name: "latin1/short",
    encoding: "latin1",
    decoder: latin1Decoder,
    payload: latin1Sample,
  },
  {
    name: "latin1/256B",
    encoding: "latin1",
    decoder: latin1Decoder,
    payload: Buffer.alloc(256, 0xe9),
  },
  {
    name: "latin1/4KiB",
    encoding: "latin1",
    decoder: latin1Decoder,
    payload: Buffer.alloc(4096, 0xe9),
  },
];
console.table(cases.map(benchmark));

const packageCount = 100_000;
const report = BenchReportView.from(native.makeReportBufferCompact(packageCount));
const { bytes, view } = report.doc;
const { strings } = report.doc;
const itemsOffset = report.packages.itemsOffset;
const itemSize = 16;

const realBufferMilliseconds = measure(() => {
  let checksum = 0;
  let itemOffset = itemsOffset;
  for (let index = 0; index < packageCount; index++, itemOffset += itemSize) {
    const id = view.getUint32(itemOffset, true);
    const [start, length] = strings.rangeUnchecked(id);
    const value = bytes.toString("utf8", start, start + length);
    checksum += value.length;
  }
  return checksum;
});
const realDecoderMilliseconds = measure(() => {
  let checksum = 0;
  let itemOffset = itemsOffset;
  for (let index = 0; index < packageCount; index++, itemOffset += itemSize) {
    const id = view.getUint32(itemOffset, true);
    const [start, length] = strings.rangeUnchecked(id);
    const value = utf8Decoder.decode(bytes.subarray(start, start + length));
    checksum += value.length;
  }
  return checksum;
});

console.log(`\nReal ZCTF workload (${packageCount.toLocaleString()} package names)`);
console.table([
  {
    method: 'Buffer.toString("utf8", start, end)',
    milliseconds: Number(realBufferMilliseconds.toFixed(3)),
  },
  {
    method: "TextDecoder.decode(bytes.subarray(start, end))",
    milliseconds: Number(realDecoderMilliseconds.toFixed(3)),
  },
  {
    method: "TextDecoder / Buffer ratio",
    milliseconds: Number((realDecoderMilliseconds / realBufferMilliseconds).toFixed(2)),
  },
]);

// Make the decoded values observable to V8.
if (sink === 0x7fff_ffff) console.log(sink);
