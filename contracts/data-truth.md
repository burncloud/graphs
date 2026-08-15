# BurnCloud Data Truth Contract

BurnCloud UI must never be more confident than its evidence.

## Required distinctions

- `UNKNOWN` is not `0`.
- `loading` is not `empty`.
- `error` is not `unavailable`.
- `configured` is not `available`.
- `available` is not `verified`.
- `receipt signed` is not `runtime attested`.
- `provider selected` is not cryptographic proof the provider executed the request.
- HTTP 4xx is not an attack signal unless an explicit rule says so.

## Source-of-truth rule

For each field shown on a migrated page, identify one of:

1. persisted authoritative backend state;
2. derived state with a documented formula and inputs;
3. sampled observation with scope/time window;
4. unknown/not implemented.

When (4) applies, show UNKNOWN/not available or omit the metric. Never create mock success values in production UI.

## Trust claims

Claims such as VERIFIED, ATTESTED, AUTHENTIC, 100%, COMPLETE, SECURE, HEALTHY require an identifiable machine-readable evidence source and failure/unknown path. If the target backend does not expose that evidence, downgrade the copy/state.
