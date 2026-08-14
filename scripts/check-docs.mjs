#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

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
  documents.set(relative, { markdown, anchors: anchorsFor(markdown) });
  const fences = markdown.split(/\r?\n/).filter((line) => /^\s*```/.test(line));
  if (fences.length % 2 !== 0) {
    errors.push(`${relative}: unbalanced fenced code block`);
  }
  if (relative.endsWith(".mbt.md")) {
    for (const [index, line] of markdown.split(/\r?\n/).entries()) {
      if (/^\s*```mbt(?:\s*)$/.test(line)) {
        errors.push(
          `${relative}:${index + 1}: use \`mbt check\` or \`mbt nocheck\` explicitly`,
        );
      }
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
