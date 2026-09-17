import { access, readFile } from "node:fs/promises";

const requiredFiles = [
  "dist/index.html",
  "dist/styles.css",
  "dist/app.js",
  "dist/favicon.png",
];

await Promise.all(requiredFiles.map((file) => access(file)));

const html = await readFile("dist/index.html", "utf8");
for (const requiredText of [
  "Embeder",
  "https://github.com/pksark06-beep/Embeder",
  "https://buymeacoffee.com/sekweb",
]) {
  if (!html.includes(requiredText)) {
    throw new Error(`Missing required site content: ${requiredText}`);
  }
}

console.log("Embeder documentation site validated.");
