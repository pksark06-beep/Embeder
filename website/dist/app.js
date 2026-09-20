// SPDX-License-Identifier: MPL-2.0
const root = document.documentElement;
const savedTheme = localStorage.getItem("embeder-theme");
const preferredLight = window.matchMedia("(prefers-color-scheme: light)").matches;
root.dataset.theme = savedTheme || (preferredLight ? "light" : "dark");

document.querySelector(".theme-toggle")?.addEventListener("click", () => {
  const next = root.dataset.theme === "light" ? "dark" : "light";
  root.dataset.theme = next;
  localStorage.setItem("embeder-theme", next);
});

// Sticky top bar gets a hairline border once the page scrolls.
const topbar = document.getElementById("topbar");
if (topbar) {
  const onScroll = () => topbar.classList.toggle("scrolled", window.scrollY > 8);
  onScroll();
  window.addEventListener("scroll", onScroll, { passive: true });
}

// Copy-to-clipboard buttons in docs code blocks.
document.querySelectorAll(".copy-button").forEach((button) => {
  button.addEventListener("click", async () => {
    const target = document.getElementById(button.dataset.copy);
    if (!target) return;
    const original = button.textContent;
    try {
      await navigator.clipboard.writeText(target.innerText);
      button.textContent = "Copied";
    } catch {
      button.textContent = "Select text";
    }
    window.setTimeout(() => { button.textContent = original; }, 1600);
  });
});

// Reveal-on-scroll for marketing sections (progressive enhancement).
const reveals = [...document.querySelectorAll(".reveal")];
if (reveals.length && "IntersectionObserver" in window && !window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
  const revealObserver = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        entry.target.classList.add("in");
        revealObserver.unobserve(entry.target);
      }
    }
  }, { rootMargin: "0px 0px -8% 0px", threshold: 0.08 });
  reveals.forEach((element) => revealObserver.observe(element));
} else {
  reveals.forEach((element) => element.classList.add("in"));
}

// Docs sidebar scroll-spy (only present on the docs page).
const navLinks = [...document.querySelectorAll(".docs-sidebar a")];
const sections = navLinks
  .map((link) => document.querySelector(link.getAttribute("href")))
  .filter(Boolean);

if (sections.length && "IntersectionObserver" in window) {
  const observer = new IntersectionObserver((entries) => {
    const visible = entries
      .filter((entry) => entry.isIntersecting)
      .sort((a, b) => b.intersectionRatio - a.intersectionRatio)[0];
    if (!visible) return;
    navLinks.forEach((link) => {
      link.classList.toggle("active", link.getAttribute("href") === `#${visible.target.id}`);
    });
  }, { rootMargin: "-18% 0px -62%", threshold: [0, 0.25, 0.6] });
  sections.forEach((section) => observer.observe(section));
}
