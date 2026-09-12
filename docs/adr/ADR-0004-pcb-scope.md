# ADR-0004 — PCB is assisted, not autonomous

- **Status:** Accepted (ratified 2026-09-12; closes OD-2)
- **Date:** 2026-09-12

## Context
The vision promises "idea-to-PCB": intent → schematic → routed, manufacturable board
→ Gerbers, autonomously. Two hard facts and one research reality:
1. KiCad **removed its integrated autorouter**; routing means external Freerouting
   (DSN/SES round-trip) or a human.
2. `pcbnew` is a **placement / DRC / plot** API — **not a router**.
3. *Electrically correct* schematic synthesis (decoupling networks, crystal load
   caps, ESP32 antenna keepout, USB/RF impedance, power integrity) from intent is
   close to an open research problem for an LLM.

## Decision
Scope PCB to **assisted, reference-design/template-driven, human-in-the-loop** for
v1. Concretely, the reliable automation is:
- `generate_netlist(graph)` from a **validated** component graph → 🟡
- `run_drc(board)` → ✅ (deterministic)
- `export_fab(board)` → Gerbers / NC drill / BOM CSV / CPL → ✅ (deterministic)

And the *non-guarantees*, surfaced as ⚠️:
- Placement from templates/reference blocks (not from first principles).
- Routing via Freerouting or the user — **never claimed as verified/manufacturable
  by Embeder.**
- Schematic electrical correctness is ⚠️ advisory, always.

## Consequences
- (+) Every PCB claim maps to an honest tier; no "looks routed, actually broken"
  boards presented as done.
- (+) Focuses effort on the parts that are genuinely reliable (netlist/DRC/export).
- (−) Does not deliver the headline "autonomous idea-to-PCB." That is reframed as a
  research track (OD-2), explicitly out of the v1 verified path.

## Alternatives rejected
- **Pursue autonomous routing for v1:** stakes the whole system's credibility on an
  unsolved problem; a plausible-but-wrong board is the most expensive failure mode
  (fabrication cost + user trust). Defer to a labeled R&D track if desired.
