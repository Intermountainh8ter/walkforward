# walkforward

**Pre-registered backtesting.** Lock the data hash, the strategy, the parameter grid, the cost model, and the in-sample / out-of-sample split in one file before any result exists. Then run walk-forward only, and report the concatenated out-of-sample record as the headline, with the deflated Sharpe ratio and the probability of backtest overfitting printed beside it whether they flatter the strategy or not.

![language: Rust](https://img.shields.io/badge/language-Rust-blue) ![license: MIT](https://img.shields.io/badge/license-MIT-blue) ![dependencies: no runtime services](https://img.shields.io/badge/data-Binance%20public%20klines-informational)

## Why

Most published backtests are the best of many tries, scored on the same data they were tuned on, with fills at prices that were not available when the signal fired. Each of those is a separate way to manufacture a Sharpe ratio. This tool removes them in order:

1. **Pre-registration first.** `prereg` writes the SHA-256 of the data file, the grid, the costs, and the split. `run` refuses to start if the data on disk no longer matches. The pre-registration is committed before the first run so the order is checkable in git history.
2. **No same-bar fills.** A signal computed on bar *t* executes at bar *t+1*'s open. Fees and slippage are charged on every fill. A test locks this in.
3. **Walk-forward only.** Parameters are chosen inside each in-sample window by a pre-stated rule and applied to the following out-of-sample window. The headline is every out-of-sample bar, in order.
4. **Overfitting diagnostics by default.** The deflated Sharpe ratio (Bailey and Lopez de Prado, 2014) discounts the best configuration's Sharpe by how many were tried. The probability of backtest overfitting (Bailey, Borwein, Lopez de Prado and Zhu, 2017) comes from combinatorially symmetric cross-validation over the full returns matrix. Both print on every run.
5. **Buy-and-hold beside it.** The same out-of-sample bars, held passively, with the same costs on the single entry.

The in-sample best configuration over the whole sample is printed too, labeled as not the headline, so a reader can see exactly how much the honest number differs from the flattering one.

## Results

Hourly Binance spot candles, 2021-01-01 to 2025-12-31 (43,810 bars per symbol), fees 10 bps and slippage 5 bps per fill, one year in-sample, one quarter out-of-sample, rolling. Pre-registrations in `results/prereg-*.json`, committed before the runs.

<!-- RESULTS:START -->
| study | OOS Sharpe | OOS return | OOS max DD | trades | buy-and-hold Sharpe | buy-and-hold return | IS-best Sharpe (not the headline) | DSR | PBO | verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| btc-donchian | 0.52 | +57.5% | 55.5% | 150 | 0.81 | +192.4% | 0.78 | 0.733 | 0.218 | not supported |
| btc-sma | 0.41 | +36.6% | 55.2% | 322 | 0.74 | +153.4% | 0.64 | 0.763 | 0.287 | not supported |
| eth-donchian | 0.26 | +7.8% | 51.6% | 82 | 0.49 | +48.4% | 0.63 | 0.695 | 0.363 | not supported |
| eth-sma | 0.23 | +0.7% | 65.6% | 336 | 0.42 | +20.8% | 0.81 | 0.858 | 0.257 | not supported |

OOS = every out-of-sample bar concatenated, parameters chosen only on the preceding in-sample window. DSR = deflated Sharpe ratio (probability the best configuration's Sharpe beats the expected maximum of unskilled trials). PBO = probability of backtest overfitting by CSCV. Full per-window detail in `results/run-*.json`.
<!-- RESULTS:END -->

## Paired flaw grader (`cleangrade`)

`cleangrade` builds matched flawed and clean copies of the simulation path on seeded synthetic series: a one-bar lookahead strategy and a Donchian breakout, 40 pairs of 3,000 bars each. Three scripted reviewers are graded against an in-run ground-truth list, and flaw recall and clean-control false-positive rate are reported separately. Results in `results/74-cleangrade.json`, run with `cargo run --locked --release -q -- cleangrade --pairs 40 --bars 3000 --seed 7 --out results/74-cleangrade.json`.

| reviewer | flaw recall | clean-control false-positive rate |
|---|---|---|
| always-flag | 1.0 | 1.0 |
| never-flag | 0.0 | 0.0 |
| truncation-invariance | 1.0 | 0.0 |

Recall alone cannot separate always-flag from the truncation reviewer; the clean controls can. The flawed strategy and the truncation check are both written by the same author, so this shows the grader works, not that the check catches every real leak. The series are synthetic random walks.

## Gate power sweep (`powersweep`)

`powersweep` plants a fixed per-bar drift in seeded synthetic random walks (100 series of 20,000 bars each, seed 7, the sma grid, 15 bps per fill, 8 CSCV blocks) and reports how often the combined gate (DSR >= 0.95 and PBO <= 0.5) passes. Results in `results/163-powersweep.json`, run with `cargo run --locked --release -q -- powersweep --series 100 --bars 20000 --seed 7 --drifts-bps 0,1,2,4,8 --out results/163-powersweep.json`.

| planted drift per bar | combined pass rate | Wilson 95% interval |
|---|---|---|
| 0 bps | 0.0 | 0.0 to 0.037 |
| 1 bps | 0.14 | 0.085 to 0.221 |
| 2 bps | 0.54 | 0.443 to 0.634 |
| 4 bps | 0.82 | 0.733 to 0.883 |
| 8 bps | 0.95 | 0.888 to 0.978 |

The pass count did not fall as drift rose. The series are synthetic i.i.d. random walks with a constant drift, which is easier to detect than a real regime-dependent edge; the result says nothing about real crypto bars.

## Leaky-signal gate test (`leakpass`)

`leakpass` runs the same sma-grid DSR and CSCV PBO gates on 100 seeded zero-drift random walks (20,000 bars each, seed 7, 15 bps per fill, 8 CSCV blocks) in two arms on identical paths: a causal signal and one computed on the next bar's close. Results in `results/165-leakpass.json`, run with `cargo run --locked --release -q -- leakpass --series 100 --bars 20000 --seed 7 --out results/165-leakpass.json`.

| arm | combined pass count (of 100) | Wilson 95% interval |
|---|---|---|
| causal | 0 | 0.0 to 0.037 |
| leaky | 3 | 0.010 to 0.085 |

The pre-registered hypothesis (leaky arm passes at least 50 of 100) was refuted. The leak here is a one-bar shift on a slow moving-average crossover, which barely changes the position, so it earns little; a stronger leak (such as the `cleangrade` flawed strategy) was not tested. The series are synthetic random walks.

## Partial look-ahead gate test (`leakfrac`)

`leakfrac` plants the full look-ahead rule from `cleanpair` on a seeded random fraction of bars (causal sma rule elsewhere) in 100 zero-drift random walks (20,000 bars each, seed 7, 15 bps per fill, 8 CSCV blocks). Results in `results/181-leakfrac.json`, run with `cargo run --locked --release -q -- leakfrac --series 100 --bars 20000 --seed 7 --fractions 0,0.02,0.05,0.1,0.25,1 --out results/181-leakfrac.json`.

| leaked bars | combined pass count (of 100) | Wilson 95% interval |
|---|---|---|
| 0 | 0 | 0.0 to 0.037 |
| 0.02 | 1 | 0.002 to 0.054 |
| 0.05 | 4 | 0.016 to 0.098 |
| 0.1 | 6 | 0.028 to 0.125 |
| 0.25 | 94 | 0.875 to 0.972 |
| 1 | 100 | 0.963 to 1.0 |

The pre-registered hypothesis (at least 50 of 100 passes at a 0.1 leak fraction) was refuted: 6 of 100 passed. The pass count rises sharply between 0.1 and 0.25. The series are synthetic random walks and the leak is one hand-written rule.

## Grid-size null sweep (`gridsweep`)

`gridsweep` reruns the zero-drift null (100 series of 20,000 bars each, seed 7, 15 bps per fill, 8 CSCV blocks) with sma grids of 4, 9, 16 and 25 configurations, the DSR trial count equal to the grid searched. Results in `results/166-gridsweep.json`, run with `cargo run --locked --release -q -- gridsweep --series 100 --bars 20000 --seed 7 --out results/166-gridsweep.json`.

| configurations | combined pass count (of 100) | PBO-only pass count (of 100) |
|---|---|---|
| 4 | 1 | 58 |
| 9 | 0 | 49 |
| 16 | 0 | 58 |
| 25 | 0 | 48 |

The pre-registered hypothesis (at most 5 of 100 at 25 configurations) held. The trial count was stated honestly here; this does not test an understated count. The series are synthetic random walks.

## Usage

```
walkforward ingest --dir <dir of data.binance.vision monthly zips> --out data/BTCUSDT-1h.csv
walkforward prereg --data data/BTCUSDT-1h.csv --strategy sma --grid 'fast=10,20,50;slow=100,200,400' \
                   --is-bars 8760 --oos-bars 2190 --out results/prereg-btc-sma.json
git add results/prereg-btc-sma.json && git commit -m "lock"      # before running
walkforward run --prereg results/prereg-btc-sma.json --out results/run-btc-sma.json
```

Strategies are deliberately plain (`sma` crossover, `donchian` breakout). The point of the repository is the procedure, not the alpha; a strategy that survives this procedure has earned the claim, and one that does not is reported with the same prominence.

## What this is not

It is not a live trading system and holds no exchange credentials. Long-flat, single asset, bar data. Intrabar fills, shorting, portfolio allocation, and borrowing costs are out of scope for this version and listed rather than approximated.

## License

MIT.

## Matched flaw and clean-control gate test (`cleanpair`)

`cleanpair` runs the DSR and CSCV PBO gates on 100 seeded zero-drift random walks (20,000 bars each, seed 7, 15 bps per fill, 8 CSCV blocks), once with a strong full-lookahead grid and once with the causal sma grid, on identical paths. Results in `results/169-cleanpair.json`, run with `cargo run --locked --release -q -- cleanpair --pairs 100 --bars 20000 --seed 7 --out results/169-cleanpair.json`.

| arm | combined pass count (of 100) | Wilson 95% interval |
|---|---|---|
| lookahead (flaw recall) | 100 | 0.963 to 1.0 |
| causal (clean-control false positives) | 0 | 0.0 to 0.037 |

The pre-registered hypothesis (recall at least 90 of 100, false positives at most 1 of 100) held. The lookahead arm peeks several bars ahead and so is far stronger than the one-bar leak in `leakpass`; any leak that earns this much is easy for the gates to pass, so this tests whether the gates can tell a strong leak from noise, not whether they detect a leak. The series are synthetic random walks.

## Embargo sweep (`embargosweep`)

`embargosweep` drops e bars at both ends of every CSCV block (in and out of sample) before the PBO step, on 100 seeded zero-drift random walks (20,000 bars each, seed 7, sma grid, 15 bps per fill, 8 CSCV blocks). Results in `results/182-embargosweep.json`, run with `cargo run --locked --release -q -- embargosweep --series 100 --bars 20000 --seed 7 --embargoes 0,24,96,336 --out results/182-embargosweep.json`.

| embargo (bars) | combined pass count (of 100) | PBO-only pass count (of 100) | Wilson 95% interval |
|---|---|---|---|
| 0 | 0 | 47 | 0.0 to 0.037 |
| 24 | 0 | 48 | 0.0 to 0.037 |
| 96 | 0 | 47 | 0.0 to 0.037 |
| 336 | 0 | 43 | 0.0 to 0.037 |

The pre-registered hypothesis (at most 3 of 100 at every embargo length) was supported. The signals here are moving averages with no labels, and the series are i.i.d. random walks with no serial dependence for an embargo to remove, so a null effect is expected; the result says nothing about real crypto bars or label-overlapping rules. The DSR step is unchanged by the embargo.


## Reality Check gate test (`realitycheck`)

`realitycheck` runs White's Reality Check (stationary bootstrap, 1000 draws, mean block length 24, zero benchmark) on the sma grid over 100 seeded zero-drift random walks (20,000 bars each, seed 7, 15 bps per fill, 8 CSCV blocks), and reports it alone and beside the DSR and PBO gates. Results in `results/183-realitycheck.json`, run with `cargo run --locked --release -q -- realitycheck --series 100 --bars 20000 --seed 7 --bootstrap-draws 1000 --mean-block 24 --out results/183-realitycheck.json`.

| gate | pass count (of 100) |
|---|---|
| Reality Check, p < 0.05 | 2 (Wilson 95% interval 0.006 to 0.070) |
| DSR only | 0 |
| PBO only | 47 |
| Reality Check and DSR | 0 |
| Reality Check and PBO | 2 |
| all three | 0 |

The pre-registered hypothesis (Reality Check passes at most 10 of 100) was supported. Power was not tested here, and the series are synthetic random walks with a nine-configuration grid.

## Volatility-clustered null gate test (`garchpass`)

`garchpass` runs the same sma-grid DSR and CSCV PBO gates on 100 seeded zero-drift series of 20,000 bars (seed 7, 15 bps per fill, 8 CSCV blocks) in two arms on the same seed: an i.i.d. random walk, and a GARCH(1,1) series with Student-t innovations. Results in `results/198-garchpass.json`, run with `cargo run --locked --release -q -- garchpass --series 100 --bars 20000 --seed 7 --out results/198-garchpass.json`.

| arm | combined pass count (of 100) | Wilson 95% interval |
|---|---|---|
| i.i.d. | 0 | 0.0 to 0.037 |
| GARCH | 0 | 0.0 to 0.037 |

The pre-registered hypothesis (GARCH arm passes at most 5 of 100) held. The GARCH parameters are one fixed choice, and the series are synthetic; the result says nothing about real crypto bars.

## Gate acceptance with planted drift, i.i.d. and GARCH (`garchpower`)

`garchpower` plants a fixed per-bar drift of 4 bps in 100 seeded series of 20,000 bars (seed 7, sma grid, 15 bps per fill, 8 CSCV blocks) in two arms on the same seed: i.i.d. and GARCH(1,1) Student-t. It counts how often the combined gate (DSR >= 0.95 and PBO <= 0.5) accepts. Results in `results/201-garchpower4bp.json`, run with `cargo run --locked --release -q -- garchpower --series 100 --bars 20000 --seed 7 --drift-bps 4 --out results/201-garchpower4bp.json`.

| arm | accepted (of 100) | Wilson 95% interval |
|---|---|---|
| i.i.d. | 82 | 0.733 to 0.883 |
| GARCH | 76 | 0.668 to 0.833 |

The pre-registered range for the GARCH arm (0.60 to 0.95) held. The i.i.d. arm equals the 4 bps row of the power sweep. The series are synthetic.

## CPCV accept rule on null series (`cpcvpass`)

`cpcvpass` splits each zero-drift series (20,000 bars, seed 7, sma grid, 15 bps per fill) into 8 blocks and holds out 2, giving 28 splits, with 24 bars dropped at both ends of every block. For each split the in-sample-best configuration is scored on the held-out blocks. A series is accepted if the 5th percentile of the 28 out-of-sample Sharpes is above zero. 100 series per arm. Results in `results/204-cpcvpass.json`, run with `cargo run --locked --release -q -- cpcvpass --series 100 --bars 20000 --seed 7 --out results/204-cpcvpass.json`.

| arm | accept count (of 100) | Wilson 95% interval |
|---|---|---|
| i.i.d. | 1 | 0.002 to 0.054 |
| GARCH(1,1) Student-t | 0 | 0.0 to 0.037 |

The pre-registered hypothesis (GARCH-arm accept count at most 10 of 100) was not refuted. Only null series were run, so this says nothing about power. The series are synthetic.

## CPCV accept rule with planted drift (`cpcvpower`)

`cpcvpower` plants a fixed per-bar drift of 2 bps in 100 seeded series of 20,000 bars (seed 7, sma grid, 15 bps per fill) in two arms, i.i.d. and GARCH(1,1) Student-t, and counts how often the CPCV accept rule from `cpcvpass` accepts (8 blocks, 2 test blocks, 28 splits, embargo 24 bars). Results in `results/205-cpcvpower.json`, run with `cargo run --locked --release -q -- cpcvpower --series 100 --bars 20000 --seed 7 --drift-bps 2 --out results/205-cpcvpower.json`.

| arm | accepted (of 100) | acceptance rate | Wilson 95% interval |
|---|---|---|---|
| i.i.d. | 67 | 0.67 | 0.573 to 0.754 |
| GARCH | 66 | 0.66 | 0.563 to 0.745 |

The pre-registered range for the GARCH arm (0.30 to 0.90) held. The series are synthetic with a constant drift; the result says nothing about real bars.

## CPCV embargo width (`cpcvembargo`)

`cpcvembargo` reruns the CPCV accept rule from `cpcvpass` on 100 seeded GARCH(1,1) Student-t series (20,000 bars each, seed 7, planted drift 2 bps per bar, 15 bps per fill, 8 blocks, 2 test blocks, 28 splits) at three embargo widths on the same series. Results in `results/206-cpcvembargo.json`, run with `cargo run --locked --release -q -- cpcvembargo --series 100 --bars 20000 --seed 7 --drift-bps 2 --embargos 0,168,480 --out results/206-cpcvembargo.json`.

| embargo | accepted (of 100) | acceptance rate |
|---|---|---|
| 0 bars | 74 | 0.74 |
| 168 bars | 61 | 0.61 |
| 480 bars | 46 | 0.46 |

The pre-registered hypothesis (acceptance at 168 bars between 0.45 and 0.80) held. Acceptance fell as the embargo widened. The series are synthetic with a constant drift.

## CPCV accept rule with a planted look-ahead (`cpcvleak`)

`cpcvleak` runs the CPCV accept rule from `cpcvpass` (8 blocks, 2 test blocks, 28 splits, embargo 24 bars) on 100 seeded zero-drift GARCH(1,1) Student-t series of 20,000 bars (seed 7, sma grid, 15 bps per fill). The look-ahead rule from `leakfrac` is planted on a random fraction of bars. Results in `results/209-cpcvleak.json`, run with `cargo run --locked --release -q -- cpcvleak --series 100 --bars 20000 --seed 7 --fractions 0,0.1,1 --out results/209-cpcvleak.json`.

| leaked bars | accepted (of 100) | Wilson 95% interval |
|---|---|---|
| 0 | 0 | 0.0 to 0.037 |
| 0.1 | 18 | 0.117 to 0.267 |
| 1 | 100 | 0.963 to 1.0 |

The pre-registered hypothesis (full-leak acceptance at least 0.50, no-leak acceptance at most 0.10) held. The accept rule does not detect a leak; it accepts the leaked series. The series are synthetic.
