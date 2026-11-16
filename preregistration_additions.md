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

## 2026-11-16T10:33:21Z leakpass proposal:9bb35b80222a94ab13ab7b72253803f03ace909770547341efc83f7c127d129a

Hypothesis: on 100 seeded zero-drift random-walk series of 20,000 bars (seed 7, sma grid, 15 bps per fill, 8 CSCV blocks), the leaky arm (sma signal computed with the next bar's close) passes the combined gate (DSR >= 0.95 and PBO <= 0.5) on at least 50 of 100 series, while the causal arm reproduces the 0 of 100 of the nullpass result.

Pre-registered metric: `/arms/leaky/combined_pass_count` in `results/165-leakpass.json`.

Expected: leaky combined pass count at least 50 of 100 (range 50 to 100); causal arm stays at 0.

Falsifier: a leaky-arm combined pass count below 50 of 100, meaning the DSR and PBO gates reject most look-ahead rules on their own.

## 2026-11-16T10:37:24Z gridsweep proposal:7ef0fa49e19ca9df550a099ec146170b6576a652942cd22a1486b4cd2cf9c340

Hypothesis: on 100 seeded zero-drift random-walk series of 20,000 bars (seed 7, 15 bps per fill, 8 CSCV blocks), the combined gate (DSR >= 0.95 and PBO <= 0.5) passes on at most 5 of 100 series at the largest grid (25 configurations), with the DSR trial count equal to the true grid size.

Pre-registered metric: `/sizes/3/combined_pass_count` in `results/166-gridsweep.json`.

Expected: combined pass count at the 25-configuration grid is at most 5 of 100 (range 0 to 5).

Falsifier: a combined pass count above 5 of 100 at the 25-configuration grid.

## 2026-11-16T10:55:36Z cleanpair proposal:d29481c5b1a83a935ec2071359b25cbda522815a56cf9fbc834e66b8e4d63e9e

Hypothesis: on 100 seeded zero-drift random-walk pairs of 20,000 bars (seed 7, 15 bps per fill, 8 CSCV blocks), the combined gate (DSR >= 0.95 and PBO <= 0.5) passes the strong full-lookahead arm on at least 90 of 100 series and passes the causal clean-control arm on at most 1 of 100 series.

Pre-registered metric: `/pairs/0/clean_false_positive_count` in `results/169-cleanpair.json`.

Expected: clean-control pass count at most 1 of 100 (range 0 to 1) while flawed-arm recall is at least 90 of 100.

Falsifier: a flawed-arm combined pass count below 90 of 100, or a clean-control combined pass count above 1 of 100.
