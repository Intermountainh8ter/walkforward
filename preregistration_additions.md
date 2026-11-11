# Pre-registration additions

## 2026-11-11T11:04:00Z cleangrade proposal:3f1150f7ac73d682d84c0ae464cb0baddaefd68c48247321aa9d8a9dc623c763

Hypothesis: on 40 seeded synthetic series of 3,000 bars, each paired as one lookahead-shifted copy and one unchanged clean control, the truncation-invariance reviewer reaches flaw recall of 1.0 and clean-control false-positive rate of 0.0, while always-flag has recall 1.0 with false-positive rate 1.0 and never-flag has recall 0.0 with false-positive rate 0.0.

Pre-registered metric: `/grader/reviewers/truncation/clean_control_false_positive_rate` in `results/74-cleangrade.json`.

Expected: the metric equals 0.0 (range 0.0 to 0.05) while always-flag equals 1.0 on the same controls.

Falsifier: the truncation-invariance reviewer flags any clean control (false-positive rate above 0.05) or misses any shifted copy (recall below 1.0).
