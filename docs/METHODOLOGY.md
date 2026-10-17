# walkforward methodology

Everything below is implemented in `src/main.rs` and covered by a unit test where a test can pin it.

## 1. Order of operations

1. `prereg` reads the data file, hashes it (SHA-256 of the bytes), expands the parameter grid, and
   writes one JSON with: data path and hash, bar count, strategy name, grid, number of configurations,
   in-sample and out-of-sample window lengths in bars, fee and slippage in basis points, bars per year,
   number of CSCV blocks, seed, the selection rule, and what the headline is. The file carries its own
   SHA-256. It is committed before any run.
2. `run` re-hashes the data and refuses to proceed if the hash or the bar count differs from the
   pre-registration.
3. Only then are returns computed.

## 2. Fill model

A strategy sees bars `0..=t` and returns a target position for the next bar (0 or 1 here; `NaN` means
hold). The target is executed at bar `t+1`'s open. If the position changes, the bar's return is the old
position's return from the previous close to this open, minus `(fee + slippage) * |change|`, plus the new
position's return from open to close. If it does not change, the return is the position's close-to-close
return. The test `signal_cannot_trade_on_its_own_bar` locks the first part; `costs_are_charged_on_every_fill`
locks the second.

Simplifications, stated: no intrabar fills, no shorting, no borrowing cost, one asset, full notional.

## 3. Walk-forward

Windows are laid end to end: `[is_start, is_start + is_bars)` for selection, then
`[is_end, is_end + oos_bars)` for evaluation, then the whole frame advances by `oos_bars`. Within each
in-sample window every configuration is simulated and the one with the highest annualized Sharpe is
chosen (ties go to the earliest in grid order). That configuration alone trades the following
out-of-sample window. The headline is the concatenation of all out-of-sample windows, each bar traded
with parameters chosen strictly before it.

Buy-and-hold over the same out-of-sample bars, with one entry cost, is reported beside it. The
pre-registered success criterion is: out-of-sample Sharpe above buy-and-hold, deflated Sharpe
probability at least 0.95, and probability of backtest overfitting at most 0.5. Anything else is
"not supported".

## 4. Metrics

Annualized Sharpe is the mean per-bar return over the sample standard deviation, times the square root
of bars per year, with no risk-free rate. Total return compounds the per-bar returns. Maximum drawdown
is the largest peak-to-trough decline of the compounded equity curve. Trades count fills.

## 5. Deflated Sharpe ratio

Bailey and Lopez de Prado (2014). For the configuration with the highest full-sample Sharpe, compute
the expected maximum Sharpe of `N` unskilled trials from the cross-trial variance of Sharpe:

```latex
SR_0 = \sqrt{V[\widehat{SR}_n]}\left((1-\gamma)\,\Phi^{-1}\!\left(1-\tfrac{1}{N}\right) + \gamma\,\Phi^{-1}\!\left(1-\tfrac{1}{N e}\right)\right)
```

with Euler's constant gamma, then the probability that the observed per-bar Sharpe exceeds it, adjusting
for the skewness and kurtosis of the selected configuration's returns:

```latex
DSR = \Phi\!\left(\frac{(\widehat{SR}-SR_0)\sqrt{T-1}}{\sqrt{1-\gamma_3\widehat{SR}+\frac{\gamma_4-1}{4}\widehat{SR}^2}}\right)
```

The normal CDF uses Abramowitz and Stegun 7.1.26; the quantile uses Acklam's approximation. A test
checks they round-trip to 1e-5, and that DSR falls as the number of trials grows.

## 6. Probability of backtest overfitting

Bailey, Borwein, Lopez de Prado and Zhu (2017), combinatorially symmetric cross-validation. The
full-sample per-bar returns of every configuration form a matrix (bars x configurations). It is cut into
`S` equal blocks (16 by default). For every way of choosing `S/2` blocks as "in-sample" (12,870 ways for
S = 16), the in-sample best configuration is found and its rank among all configurations on the
complementary "out-of-sample" blocks is recorded as a relative rank `w = rank / (N + 1)` and a logit
`ln(w / (1 - w))`. PBO is the share of logits at or below zero: how often the in-sample winner is
out-of-sample median or worse. A test on deterministic pseudo-noise checks PBO sits near 0.5, as it
should when there is nothing to find.

## 7. What the four 2026-10-17 studies mean

None of the four passed. Trend-following rules on hourly BTC and ETH, tuned yearly and traded
quarterly with 15 bps round-trip cost, returned less than holding the asset in every case, and the gap
between each strategy's full-sample best Sharpe and its honest walk-forward Sharpe (0.64 to 0.41, 0.81
to 0.23, and so on) is the quantity this tool exists to make visible. That is reported as the finding.
