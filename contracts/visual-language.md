# BurnCloud UI Visual Contract

The React reference in `rustburn/burncloud-ui` is the visual reference, not the data source.

## Preserve

- Calm light-gray application shell and white content surfaces.
- Inter-like sans typography, strong display headings, monospace for technical identifiers.
- Precise 1px borders, restrained shadows, consistent spacing and alignment.
- Small semantic status accents: green = positive/pass, amber = attention/degraded, red = failure, gray = unknown/neutral.
- Compact tables, drawers for focused editing, strong hover/focus states.
- Existing BurnCloud Dioxus shell/components when they already express the same visual primitive.

## Do not cargo-cult

- Do not recreate every React Card just because it exists in the reference.
- Do not copy marketing claims, simulated telemetry, fake timers, random latency, fake cryptographic proofs, or mock business numbers.
- Do not add gradients/glows merely to appear technical.
- Do not convert one source component into one target component mechanically.

## Page hierarchy

A page must visually communicate in this order:

1. Page title and responsibility.
2. Current conclusion/status when the backend can support it.
3. Primary work surface.
4. Attention/error state when needed.
5. Details via row expansion/drawer/navigation.

One block should communicate one conclusion. Prefer semantic compression over KPI-card proliferation.
