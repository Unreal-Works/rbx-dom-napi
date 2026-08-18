"use strict";

const { spawnSync } = require("node:child_process");
const path = require("node:path");

const npm = process.platform === "win32" ? "npm.cmd" : "npm";
const root = path.resolve(__dirname, "..");
const env = { ...process.env };
for (const key of Object.keys(env)) {
  if (key.toLowerCase().startsWith("npm_config_")) delete env[key];
}

for (const example of ["basic-consumer", "format-converter"]) {
  const cwd = path.join(root, "examples", example);
  for (const args of [["install", "--ignore-scripts"], ["test"]]) {
    const result = spawnSync(npm, args, { cwd, env, stdio: "inherit" });
    if (result.status !== 0) process.exit(result.status ?? 1);
  }
}
