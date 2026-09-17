import { access, readFile } from "node:fs/promises";

const requiredFiles = [
  "dist/index.html",
  "dist/docs.html",
  "dist/styles.css",
  "dist/app.js",
  "dist/favicon.png",
  "dist/favicon.ico",
  "dist/img/logo.png",
  "dist/fonts/Geist.woff2",
  "dist/fonts/GeistMono.woff2",
  "dist/img/app-build-verified.png",
  "dist/img/app-datasheet.png",
  "dist/img/app-simulation.png",
  "dist/img/app-editor.png",
  "dist/img/app-extensions.png",
];

await Promise.all(
  requiredFiles.map((file) =>
    access(file).catch(() => {
      throw new Error(`Missing required site asset: ${file}`);
    }),
  ),
);

const home = await readFile("dist/index.html", "utf8");
for (const requiredText of [
  "Embeder",
  "https://github.com/pksark06-beep/Embeder",
  "https://buymeacoffee.com/sekweb",
  "./img/app-build-verified.png",
  "/docs",
]) {
  if (!home.includes(requiredText)) {
    throw new Error(`Missing required content in index.html: ${requiredText}`);
  }
}

const docs = await readFile("dist/docs.html", "utf8");
for (const requiredText of ["Documentation", "id=\"install\"", "id=\"byok\"", "id=\"architecture\""]) {
  if (!docs.includes(requiredText)) {
    throw new Error(`Missing required content in docs.html: ${requiredText}`);
  }
}

console.log("Embeder documentation site validated.");
