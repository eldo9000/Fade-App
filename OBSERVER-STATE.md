# Fade-App Observer State
Last updated: 2026-05-05  ·  Run 20

---

## Active development areas

The single landed commit since Run 19 is `ad281ec`, a picker-side feature that wires a `building: true` flag through the format-group registry in `src/App.svelte` and audits every conversion/tools/files group against the 0.6.5 and 0.6.6 sweep baselines. Live marks now reflect what the sweeps actually verified; building marks cover env-blocked codecs (hap, theora, ogv), managed-install dependencies (font, parquet, ipynb, timeline, usd/usdz/abc/blend), and untested categories (image-seq, archive iso/dmg/cbr/cbz/rar). The email group was promoted from todo to live on the strength of the extra_sweep eml/mbox cases. The change is purely a registry/UX edit — no codec, validator, or argument-builder code was touched.

In-flight but unlanded: a design-system pass on `common-js/`. Four new component primitives (`Checkbox.svelte`, `SectionLabel.svelte`, `SegmentedControl.svelte`, `Select.svelte`) sit untracked alongside modified `common-js/src/index.js`, `common-js/src/tokens.css`, and `src/app.css`. This is shared-primitive work and crosses into the Libre-Apps design language. It has not been committed and has not been through CI.

## Fragile / high-risk areas

Nothing notable. The fragility band that was active through 0.6.4–0.6.6 (BC-005 encoder-constraint churn in `args/video.rs`) is fully cooled: nine documented instances, all CONFIRMED, with the DNxHR arg-builder concern resolved at the contract layer. The 0.7-prep Blender hardening arc has landed end-to-end — locate_script and find_blender now have unit tests on path logic (555e602, 851b55d) and the stale risk entries closed in 2a27387. The only remaining OPEN log entries are environmental (HAP×2, libtheora absent from Homebrew FFmpeg 8.1) and have been stable since 2026-04-28. No code-shaped OPEN entries remain.

## Deferred work accumulation

The three environmental OPEN entries (HAP encoder absent, HAP resolution divisibility unverifiable, libtheora absent) carry forward unchanged from Run 18 — they are blocked on toolchain availability, not on engineering attention. The h265-lossless deferral and DNxHD/DNxHR 64×64 fixture-shape failures remain expected in the sweep baseline.

`SESSION-STATUS.md` Known Risks still lists "no unit tests for path logic (TASK-29)" as a remaining gap, which is now stale — that work landed in 555e602 and 851b55d on 2026-04-29. This stale forward reference is unchanged from Run 19's observation. SESSION-STATUS itself has not been updated since 2026-04-29 despite the picker commit landing the next day.

The deleted task files (TASK-12, TASK-13, TASK-14) and the `scripts/computer-use/__pycache__` plus six fixture files (test.csv/jpg/json/mp4/png/wav/zip) continue to drift in the working tree as untracked or unstaged. The drift is unchanged from Run 19 — it is background noise, not accumulation, but it has now persisted across two observer runs without resolution.

## Pattern watch

BC-005 instance count holds at nine; no new instance surfaced this run, and no doc-side backfill landed since Run 19. None of BC-001 through BC-004 produced new sightings. The `$bindable` write-back cascade gotcha and the `validate_output_name()` requirement remain documented and unbreached. The picker commit operates entirely in declarative registry data and presents no surface where any current Known Bug Class would apply.

## CI health

Net green. The five visible runs are four successes and one failure; the failure (25140270914, the 49-second rustfmt slip on the Blender unit-test commit) is the same one Run 19 triaged and closed, not a new break. The picker commit itself ran clean (25141660233, 3m45s) on 2026-04-30. The most recent run is a scheduled `Cleanup old releases` housekeeping job from 2026-05-03 that completed in 16s. No flakes, no recurrences, no red runs in the last six days.

## Observer notes

Momentum has slowed sharply. One landed commit in the six days since Run 19, against a backdrop of three sprints and twenty-one tasks closed in the four days before that. The slowdown is consistent with the documented release posture — 0.6 → 0.8 is a testing-heavy window, not a feature-heavy one — and with the fact that the headline arcs (BC-005, image validation, video re-baseline, Blender hardening) all closed at once. There is no arc in flight.

Two protocol gaps from Run 19 persist unchanged. Git notes remain absent on every commit in the last 40 — the commit-rationale-note protocol has now been unused for thirteen-plus consecutive observer runs and should be considered de facto retired unless someone formally revives it. SESSION-STATUS.md still carries the stale TASK-29 reference under Known Risks despite the work having landed; the document has not been refreshed since 2026-04-29.

The unlanded design-system work in `common-js/` is the one signal worth watching. Four new shared primitives plus modifications to `tokens.css`, `index.js`, and `src/app.css` is a non-trivial diff sitting outside version control across multiple observer runs. Because `common-js` is shared infrastructure with the Libre-Apps line, drift here has cross-product implications, not just Fade ones. The diff is not a risk yet — it is unlanded, unbroken, and CI-untouched — but it is the largest piece of unrepresented state in the working tree and it has been parked for at least one full run.

Run 19's flagged risks have all shrunk or resolved: the Blender unit-test gap closed in commits, the rustfmt slip triaged, BC-005 quiet. Nothing new has grown to replace them.
