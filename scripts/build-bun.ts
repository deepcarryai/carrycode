import { build } from "bun";
import { join, dirname } from "path";
import { copyFile, mkdir, readdir } from "fs/promises";
import { existsSync, rmSync } from "fs";

async function main() {
  const rootDir = join(import.meta.dir, "..");
  const distDir = join(rootDir, "dist-bun");
  const targetDir = join(rootDir, "target");

  const args = process.argv.slice(2);
  const targetArg = args.find(a => a.startsWith("--target="));
  // Default to linux-x64 if not specified (or should we default to host?)
  // Common targets: bun-linux-x64, bun-darwin-arm64, bun-darwin-x64, bun-windows-x64
  const target = targetArg ? targetArg.split("=")[1] : "bun-linux-x64";

  console.log(`🚀 Starting Bun build for target: ${target}`);

  // Determine native module suffix based on target
   let nativeSuffix = ".node";
   if (target.includes("darwin-arm64")) {
     nativeSuffix = ".darwin-arm64.node";
   } else if (target.includes("darwin-x64")) {
     nativeSuffix = ".darwin-x64.node";
   } else if (target.includes("windows")) {
     nativeSuffix = ".win32-x64-msvc.node";
   } else if (target.includes("linux-x64-musl")) {
     nativeSuffix = ".linux-x64-musl.node";
   } else if (target.includes("linux-x64-gnu")) {
     nativeSuffix = ".linux-x64-gnu.node";
   } else if (target.includes("linux-arm64")) {
     nativeSuffix = ".linux-arm64-gnu.node";
   } else if (target.includes("linux-x64")) {
      // Default linux usually gnu
     nativeSuffix = ".linux-x64-gnu.node";
   }

   // Clean dist
   const targetDistDir = join(rootDir, `dist-${target}`);
   if (existsSync(targetDistDir)) {
     rmSync(targetDistDir, { recursive: true });
   }
   await mkdir(targetDistDir, { recursive: true });

   // 1. Find and copy native module
   console.log(`📦 Locating native module (suffix: ${nativeSuffix})...`);
  let nodeFile: string | null = null;
  
  // Helper to find best match
  const findMatch = (files: string[]) => {
      // 1. Try exact match with suffix
      const exact = files.find(f => f.endsWith(nativeSuffix));
      if (exact) return exact;
      // 2. If no target specified (default build), take any .node
      if (!targetArg) {
          return files.find(f => f.endsWith('.node'));
      }
      return null;
  };

  // Try to find in target/ first
  if (existsSync(targetDir)) {
    const files = await readdir(targetDir);
    const found = findMatch(files);
    if (found) {
      nodeFile = join(targetDir, found);
    }
  }

  // Fallback to searching in root
  if (!nodeFile) {
    const files = await readdir(rootDir);
    const found = findMatch(files);
    if (found) {
      nodeFile = join(rootDir, found);
    }
  }

  if (!nodeFile) {
    console.error(`❌ Error: Native module (*${nativeSuffix}) not found. Please run 'npm run build:rust' (with appropriate target) first.`);
    process.exit(1);
  }

  console.log(`✅ Found native module: ${nodeFile}`);
  const destNodeFile = join(targetDistDir, "core.lib");
  await copyFile(nodeFile, destNodeFile);
  console.log(`📋 Copied to ${destNodeFile} (renamed to core.lib)`);

  // Read version from package.json
  const pkgPath = join(rootDir, "package.json");
  const pkgContent = await Bun.file(pkgPath).text();
  const pkg = JSON.parse(pkgContent);
  const version = pkg.version || "0.0.0";
  console.log(`📌 Version: ${version}`);

  // 2. Build TS with Bun
  console.log("🔨 Compiling TypeScript with Bun...");
  
  // Actually, Bun.build() produces JS files. For --compile (single binary), 
  // currently the recommended way is to use `bun build --compile` CLI.
  
  const proc = Bun.spawn([
    "bun", 
    "build", 
    "--compile",
    "--minify",
    "--sourcemap=none",
    `--target=${target}`, 
  `--define`, `CARRYCODE_VERSION="${version}"`,
    join(rootDir, "src-ts/index.tsx"),
    "--outfile", 
  join(targetDistDir, "carry")
  ], {
    stdout: "inherit",
    stderr: "inherit"
  });

  const exitCode = await proc.exited;

  if (exitCode !== 0) {
    console.error("❌ Bun compilation failed.");
    process.exit(exitCode);
  }

  console.log("✅ Bun compilation successful!");
  console.log(`\n🎉 Distribution ready in ${targetDistDir}/`);
  console.log(`   - carry (Executable)`);
  console.log(`   - core.lib (Native Module)`);
}

main();
