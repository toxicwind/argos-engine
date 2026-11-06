// lapidary engine rename runner — the eval's execFileSync works with node:child_process
// but the in-process one spawns without PATH sometimes. Run with a patched process.env.
import { execSync } from "node:child_process";

const ENG = "/home/toxic/pens/agentic-moment/products/argos/engine";
process.env.PATH = "/usr/bin:/bin:/usr/local/bin:" + (process.env.PATH || "");

// Run lapidary in a subprocess where PATH is definitely set (not execFileSync inside bun runtime)
const r = Bun.spawnSync(["bun", "/home/toxic/pens/geode/products/lapidary/bin/lapidary.ts",
  "--repo", `${ENG}/crates`,
  "--rules", `${ENG}/.argos-rename.lapidary`,
  "--dry-run"], {
  cwd: ENG,
  env: { ...process.env, PATH: "/usr/bin:/bin:/usr/local/bin:" + process.env.PATH },
  stdout: "pipe",
  stderr: "pipe",
});
const out = Buffer.concat([r.stdout ?? [], r.stderr ?? []]).toString("utf8");
console.log("exitCode:", r.exitCode);
console.log("bytes:", out.length);
console.log(out.slice(-6000));
