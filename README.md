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
