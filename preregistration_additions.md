# Pre-registration additions

## 2026-11-11T11:04:00Z cleangrade proposal:3f1150f7ac73d682d84c0ae464cb0baddaefd68c48247321aa9d8a9dc623c763

Hypothesis: on 40 seeded synthetic series of 3,000 bars, each paired as one lookahead-shifted copy and one unchanged clean control, the truncation-invariance reviewer reaches flaw recall of 1.0 and clean-control false-positive rate of 0.0, while always-flag has recall 1.0 with false-positive rate 1.0 and never-flag has recall 0.0 with false-positive rate 0.0.

Pre-registered metric: `/grader/reviewers/truncation/clean_control_false_positive_rate` in `results/74-cleangrade.json`.

Expected: the metric equals 0.0 (range 0.0 to 0.05) while always-flag equals 1.0 on the same controls.

Falsifier: the truncation-invariance reviewer flags any clean control (false-positive rate above 0.05) or misses any shifted copy (recall below 1.0).

## 2026-11-16T10:19:48Z nullpass proposal:1ec4ddc0cba05570b9b1e105a662b47241a3b07e78801e990bfacd8290bfd36e

Hypothesis: on 100 seeded zero-drift random-walk series of 20,000 hourly bars each, with the sma grid fast=10,20,50;slow=100,200,400, fee 10 bps plus slippage 5 bps per fill (15 bps), and 8 CSCV blocks, at most 10 series pass the combined gate DSR >= 0.95 and PBO <= 0.5 (both computed on the full sample, as in `run`).

Pre-registered metric: `/gates/null_pass_count` in `results/162-nullpass.json`.

Expected: the metric is at most 10 of 100 (range 0 to 10). A Wilson 95% interval is reported beside it.

Falsifier: 11 or more of the 100 noise series pass both gates, meaning the combined gate has a false-positive rate above 10% on noise.

## 2026-11-16T10:23:38Z powersweep proposal:249b648e6dcce38b9c25e6fca8694f8514b326c7bc5fdee480bc4e7b8e4b3b61

Hypothesis: on seeded 20,000-bar hourly synthetic series with planted per-bar drift of 0, 1, 2, 4 and 8 basis points (100 series per size, seed 7, the same sma grid, 15 bps cost per fill and 8 CSCV blocks as iteration 162), the combined gate DSR >= 0.95 and PBO <= 0.5 passes at least 50 of 100 series at the largest drift, and the pass count never decreases as drift grows.

Pre-registered metric: `/gates/power_by_effect_size/8bps/pass_rate` in `results/163-powersweep.json`.

Expected: the pass rate at 8 bps per bar drift is at least 0.50 (range 0.5 to 1.0), above the pass rate at zero drift.

Falsifier: a pass rate below 0.50 at the largest drift, or a pass count that falls as drift rises, or a nonzero pass count at zero drift beyond the iteration 162 result.
