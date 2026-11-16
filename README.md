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
