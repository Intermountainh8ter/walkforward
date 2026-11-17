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

## 2026-11-16T12:07:43Z leakfrac proposal:28f451aaba863e3dbc208a0b0dac1f0ff2826a012e5ea7eca413f503888b9b8c

Hypothesis: on 100 seeded zero-drift random-walk series of 20,000 bars (seed 7, 15 bps per fill, 8 CSCV blocks, grid fast=10,20,50 slow=100,200,400), a look-ahead rule active on 10 percent of bars (chosen by a seeded per-bar draw, otherwise the causal sma rule) passes the combined gate (DSR >= 0.95 and PBO <= 0.5) on at least 50 of 100 series, while the 0 percent control passes on at most 1 of 100.

Pre-registered metric: `/fractions/3/combined_pass_count` in `results/181-leakfrac.json`.

Expected: combined pass count at the 10 percent leak fraction at least 50 of 100 (range 50 to 100).

Falsifier: a combined pass count below 50 of 100 at the 10 percent leak fraction, or a zero-leak control pass count above 1 of 100.

## 2026-11-16T12:13:41Z embargosweep proposal:5c98ee87dec3ceb272162c2836b23a8ad0b9c45b6f7a70bfdb01218091027376

Hypothesis: on 100 seeded zero-drift random-walk series of 20,000 bars (seed 7, sma grid fast=10,20,50 slow=100,200,400, 15 bps per fill, 8 CSCV blocks), dropping e bars at each end of every CSCV block (an embargo on each side of every block boundary) leaves the combined gate (DSR >= 0.95 and PBO <= 0.5) passing at most 3 of 100 series at every embargo length e in 0, 24, 96 and 336 bars, so the embargo does not change the null pass count by more than 3.

Pre-registered metric: `/embargo_sweep/0/null_pass_count` in `results/182-embargosweep.json`.

Expected: null pass count at embargo 0 is at most 3 of 100 (range 0 to 3), and no embargo length differs from it by more than 3.

Falsifier: any embargo length with a combined-gate pass count above 3 of 100, or counts at two embargo lengths differing by more than 3.

## 2026-11-16T12:18:43Z realitycheck proposal:5a44b29e90c7390b316078d27e23c3d0d8fa5fe9357e820ff8841f6c77ed743c

Hypothesis: on 100 seeded zero-drift random walks (20,000 bars, sma grid, 15 bps per fill, 1000 stationary-bootstrap draws, mean block length 24), White's Reality Check at p < 0.05 passes at most 10 of 100 series, so it does not reject more spurious rules than its nominal size allows.

Pre-registered metric: `/reality_check/null_pass_count` in `results/183-realitycheck.json`.

Expected: null pass count at most 10 of 100 (range 0 to 10).

Falsifier: a Reality Check null pass count above 10 of 100.

## 2026-11-17T13:01:27Z garchpass proposal:0883b8a5f9d9dd4befc75baf52e93cdbaed1e5c1eeb8d04aabead9276131d5f4

Hypothesis: on 100 seeded zero-drift GARCH(1,1) Student-t series of 20,000 bars (seed 7, sma grid fast=10,20,50 slow=100,200,400, 15 bps per fill, 8 CSCV blocks), the combined gate (DSR >= 0.95 and PBO <= 0.5) passes at most 5 of 100 series, so volatility clustering does not raise the null pass count above the i.i.d. arm's range by more than 5. Arm 0 is the i.i.d. arm on the same seed, arm 1 is the GARCH arm.

Pre-registered metric: `/arms/1/null_pass_count` in `results/198-garchpass.json`.

Expected: GARCH arm null pass count at most 5 of 100 (range 0 to 5).

Falsifier: a GARCH-arm combined-gate pass count above 5 of 100.

## 2026-11-17T13:15:38Z garchpower proposal:21627f58c651edb0cc5508dfee148c65abec3790f2a9d0360014c41679bfa261

Hypothesis: on 100 seeded series of 20,000 bars (seed 7, sma grid fast=10,20,50 slow=100,200,400, 15 bps per fill, 8 CSCV blocks) with a planted drift of 4 bps per bar, the combined gate (DSR >= 0.95 and PBO <= 0.5) accepts between 60 and 95 percent of GARCH(1,1) Student-t series, so volatility clustering does not collapse acceptance relative to the 0.82 i.i.d. rate in results/163-powersweep.json. Arm 0 is i.i.d., arm 1 is GARCH, same seed.

Pre-registered metric: `/arms/1/acceptance_rate` in `results/201-garchpower4bp.json`.

Expected: GARCH-arm acceptance rate at 4 bps lies between 0.60 and 0.95.

Falsifier: a GARCH-arm acceptance rate below 0.60 or above 0.95 in the run.

## 2026-11-17T13:30:24Z cpcvpass proposal:4283ba10e679c8ee02230a8ed5acb10a9db9ed872044fb42172e2841e591ac6c

Hypothesis: on 100 seeded zero-drift GARCH(1,1) Student-t series of 20,000 bars (seed 7, sma grid fast=10,20,50 slow=100,200,400, 15 bps per fill, 8 blocks, 2 test blocks, 28 splits, embargo 24 bars), a CPCV accept rule (in-sample-best sma configuration per split, accept if the 5th percentile of out-of-sample split Sharpes is above zero) accepts at most 10 of 100 series. Arm 0 is i.i.d., arm 1 is GARCH, same seed.

Pre-registered metric: `/arms/1/null_pass_count` in `results/204-cpcvpass.json`.

Expected: GARCH-arm CPCV accept count at most 10 of 100 (range 0 to 10).

Falsifier: a GARCH-arm CPCV accept count above 10 of 100.

## 2026-11-17T13:35:24Z cpcvpower proposal:fd82758df270d7a628ea1550deb5aed35b99d9118056be329812f6a6b49dcd3c

Hypothesis: with a planted drift of 2 bps per bar on 100 seeded series of 20,000 bars (seed 7, sma grid fast=10,20,50 slow=100,200,400, 15 bps per fill, 8 blocks, 2 test blocks, 28 splits, embargo 24 bars), the CPCV accept rule (5th percentile of out-of-sample split Sharpes of the in-sample-best configuration above zero) accepts between 30 and 90 percent of GARCH(1,1) Student-t series. Arm 0 is i.i.d., arm 1 is GARCH, same seed.

Pre-registered metric: `/arms/1/acceptance_rate` in `results/205-cpcvpower.json`.

Expected: GARCH-arm CPCV acceptance rate at 2 bps lies between 0.30 and 0.90.

Falsifier: a GARCH-arm acceptance rate below 0.30 or above 0.90 in the run.

## 2026-11-17T13:39:50Z cpcvembargo proposal:9a01a60408fe9074cc7e187f81c06c8414b3a2b80aa9f152d806643e8e8938fd

Hypothesis: on 100 seeded GARCH(1,1) Student-t series of 20,000 bars with a planted drift of 2 bps per bar (seed 7, sma grid fast=10,20,50 slow=100,200,400, 15 bps per fill, 8 blocks, 2 test blocks, 28 splits), the CPCV accept rule at an embargo of 168 bars accepts between 0.45 and 0.80 of series. Arms are ordered by embargo: 0, 168, 480 bars.

Pre-registered metric: `/arms/1/acceptance_rate` in `results/206-cpcvembargo.json`.

Expected: acceptance rate at embargo 168 bars lies between 0.45 and 0.80.

Falsifier: an acceptance rate at embargo 168 bars below 0.45 or above 0.80.

## 2026-11-17T13:54:25Z cpcvleak proposal:d1ef12416c6c271b405c6d6692e2e7887c32eee7b2a941aa553b705e7076faeb

Hypothesis: on 100 seeded zero-drift GARCH(1,1) Student-t series of 20,000 bars (seed 7, sma grid, 15 bps per fill, 8 blocks, 2 test blocks, embargo 24 bars), the CPCV accept rule accepts at least 0.50 of series when the look-ahead rule is planted on every bar (arm 2), while accepting at most 0.10 with no leak (arm 0). Arms are leaked fractions 0, 0.1, 1.

Pre-registered metric: `/arms/2/acceptance_rate` in `results/209-cpcvleak.json`.

Expected: full-leak arm acceptance rate at least 0.50.

Falsifier: a full-leak arm acceptance rate below 0.50, or a no-leak arm rate above 0.10.
