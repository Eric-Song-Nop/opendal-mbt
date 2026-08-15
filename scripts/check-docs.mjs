#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageListFlag = process.argv.indexOf("--package-list");
let packagedFiles = null;
if (packageListFlag !== -1) {
  const packageListPath = process.argv[packageListFlag + 1];
  if (!packageListPath) {
    throw new Error("--package-list requires a path");
  }
  const packageList = await fs.readFile(packageListPath, "utf8");
  packagedFiles = new Set(
    packageList.split(/\r?\n/).map((line) => line.trim()).filter(Boolean),
  );
}

async function collectMarkdown(relativeRoot, predicate) {
  const absoluteRoot = path.join(repoRoot, relativeRoot);
  const found = [];
  async function visit(directory) {
    for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
      const absolute = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        if (
          entry.name.startsWith(".") ||
          entry.name === "_build" ||
          entry.name === "node_modules" ||
          entry.name === "target"
        ) {
          continue;
        }
        await visit(absolute);
      } else if (entry.isFile() && predicate(entry.name)) {
        found.push(path.relative(repoRoot, absolute));
      }
    }
  }
  await visit(absoluteRoot);
  return found;
}

const files = [
  ...(await collectMarkdown("src", (name) => name.endsWith(".mbt.md"))),
  ...(await collectMarkdown("docs", (name) => name.endsWith(".md"))),
  ...(await collectMarkdown("examples", (name) => name === "README.md")),
  "wasm/README.md",
].sort();

const errors = [];
const documents = new Map();

function githubSlug(text) {
  return text
    .replace(/<[^>]*>/g, "")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/[`*_~]/g, "")
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s+/g, "-");
}

function analyzeMarkdown(markdown) {
  const visibleLines = [];
  const bareMbtFenceLines = [];
  let fence = null;

  for (const [index, line] of markdown.split(/\r?\n/).entries()) {
    if (fence !== null) {
      const closing = line.match(/^ {0,3}(`{3,}|~{3,})[ \t]*$/);
      if (
        closing !== null &&
        closing[1][0] === fence.marker &&
        closing[1].length >= fence.length
      ) {
        fence = null;
      }
      visibleLines.push("");
      continue;
    }

    const opening = line.match(/^ {0,3}(`{3,}|~{3,})(.*)$/);
    if (
      opening === null ||
      (opening[1][0] === "`" && opening[2].includes("`"))
    ) {
      visibleLines.push(line);
      continue;
    }

    fence = { marker: opening[1][0], length: opening[1].length };
    if (opening[2].trim() === "mbt") {
      bareMbtFenceLines.push(index + 1);
    }
    visibleLines.push("");
  }

  return {
    visibleMarkdown: visibleLines.join("\n"),
    bareMbtFenceLines,
    hasUnclosedFence: fence !== null,
  };
}

function anchorsFor(markdown) {
  const anchors = new Set();
  const occurrences = new Map();
  for (const line of markdown.split(/\r?\n/)) {
    const heading = line.match(/^#{1,6}\s+(.+?)\s*#*\s*$/);
    if (heading) {
      const base = githubSlug(heading[1]);
      const count = occurrences.get(base) ?? 0;
      occurrences.set(base, count + 1);
      anchors.add(count === 0 ? base : `${base}-${count}`);
    }
    for (const match of line.matchAll(/<(?:a|[A-Za-z][^>]*)\s+(?:id|name)=["']([^"']+)["'][^>]*>/g)) {
      anchors.add(match[1]);
    }
  }
  return anchors;
}

for (const relative of files) {
  const markdown = await fs.readFile(path.join(repoRoot, relative), "utf8");
  const analysis = analyzeMarkdown(markdown);
  documents.set(relative, {
    markdown: analysis.visibleMarkdown,
    anchors: anchorsFor(analysis.visibleMarkdown),
  });
  if (analysis.hasUnclosedFence) {
    errors.push(`${relative}: unbalanced fenced code block`);
  }
  if (relative.endsWith(".mbt.md")) {
    for (const line of analysis.bareMbtFenceLines) {
      errors.push(
        `${relative}:${line}: use \`mbt check\` or \`mbt nocheck\` explicitly`,
      );
    }
  }
}

function normalizeDestination(raw) {
  let destination = raw.trim();
  if (destination.startsWith("<")) {
    const end = destination.indexOf(">");
    if (end !== -1) destination = destination.slice(1, end);
  } else {
    destination = destination.split(/\s+["']/u, 1)[0];
  }
  return destination.replace(/&amp;/g, "&");
}

function localDestination(destination) {
  return !(
    destination === "" ||
    destination.startsWith("/") ||
    /^[A-Za-z][A-Za-z0-9+.-]*:/.test(destination)
  );
}

for (const [relative, { markdown }] of documents) {
  if (packagedFiles !== null && !packagedFiles.has(relative)) continue;
  const destinations = [];
  for (const match of markdown.matchAll(/!?\[[^\]]*\]\(([^)\n]+)\)/g)) {
    destinations.push({ raw: match[1], offset: match.index });
  }
  for (const match of markdown.matchAll(/^\s*\[[^\]]+\]:\s*(\S+)/gm)) {
    destinations.push({ raw: match[1], offset: match.index });
  }
  const lineAt = (offset) => markdown.slice(0, offset).split(/\r?\n/).length;
  for (const { raw, offset } of destinations) {
    const destination = normalizeDestination(raw);
    if (!localDestination(destination)) continue;
    const [encodedPath, encodedAnchor] = destination.split("#", 2);
    const decodedPath = decodeURIComponent(encodedPath.split("?", 1)[0]);
    const target = decodedPath === ""
      ? relative
      : path.normalize(path.join(path.dirname(relative), decodedPath));
    let stat;
    try {
      stat = await fs.stat(path.join(repoRoot, target));
    } catch {
      errors.push(`${relative}:${lineAt(offset)}: missing local link target ${destination}`);
      continue;
    }
    const packagedTarget = target.split(path.sep).join("/");
    const targetIsPackaged = packagedFiles === null ||
      packagedFiles.has(packagedTarget) ||
      (stat.isDirectory() &&
        [...packagedFiles].some((file) => file.startsWith(`${packagedTarget}/`)));
    if (!targetIsPackaged) {
      errors.push(
        `${relative}:${lineAt(offset)}: local link target is not published: ${destination}`,
      );
      continue;
    }
    if (encodedAnchor && stat.isFile() && documents.has(target)) {
      const anchor = decodeURIComponent(encodedAnchor).toLowerCase();
      if (!documents.get(target).anchors.has(anchor)) {
        errors.push(`${relative}:${lineAt(offset)}: missing anchor #${anchor} in ${target}`);
      }
    }
  }
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}

console.log(`documentation check passed: ${files.length} canonical Markdown files`);
