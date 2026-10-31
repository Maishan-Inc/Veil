import { copyFile, mkdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const from = resolve("src/index.js");
const to = resolve("dist/index.js");

await mkdir(dirname(to), { recursive: true });
await copyFile(from, to);
