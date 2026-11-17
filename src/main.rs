//! walkforward: pre-registered backtesting.
//!
//! The order of operations is the whole point. `prereg` hashes the data file,
//! the strategy, the parameter grid, the fee model and the in-sample /
//! out-of-sample split into one locked JSON before any result exists. `run`
//! refuses to start unless the data on disk still matches that hash, selects
//! parameters only inside each in-sample window, applies them to the following
//! out-of-sample window, and reports the concatenated out-of-sample record as
//! the headline. In-sample numbers are printed, labelled, and never alone.
//!
//! Two overfitting diagnostics come with every run: the deflated Sharpe ratio
//! (Bailey & Lopez de Prado 2014) for the best configuration given how many
//! were tried, and the probability of backtest overfitting (Bailey, Borwein,
//! Lopez de Prado & Zhu 2017) via combinatorially symmetric cross-validation.
//!
//! Fills happen at the next bar's open. A signal computed on bar t cannot
//! trade at bar t's close. Fees and slippage are charged on every fill.

use clap::{Parser, Subcommand};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "walkforward", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Convert Binance monthly kline zips (data.binance.vision) in a directory into one sorted CSV.
    Ingest {
        #[arg(long)]
        dir: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Write the locked pre-registration. Nothing is computed.
    Prereg {
        #[arg(long)]
        data: PathBuf,
        /// sma | donchian
        #[arg(long)]
        strategy: String,
        /// Grid as name=a,b,c;name2=x,y (every combination is one configuration).
        #[arg(long)]
        grid: String,
        #[arg(long)]
        is_bars: usize,
        #[arg(long)]
        oos_bars: usize,
        #[arg(long, default_value_t = 10.0)]
        fee_bps: f64,
        #[arg(long, default_value_t = 5.0)]
        slippage_bps: f64,
        /// Bars per year for annualisation (8760 for hourly).
        #[arg(long, default_value_t = 8760.0)]
        bars_per_year: f64,
        #[arg(long, default_value_t = 16)]
        cscv_blocks: usize,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run the pre-registered study. Refuses if the data hash has changed.
    Run {
        #[arg(long)]
        prereg: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Render every `run-*.json` in a directory as one Markdown table and splice it into the README
    /// between `<!-- RESULTS:START -->` and `<!-- RESULTS:END -->`. Refuses on a missing field.
    Report {
        #[arg(long)]
        dir: PathBuf,
        #[arg(long)]
        readme: PathBuf,
    },
    /// Grade three scripted reviewers on matched flawed (lookahead) and clean copies of synthetic
    /// series; report flaw recall and clean-control false-positive rate separately.
    Cleangrade {
        #[arg(long, default_value_t = 40)]
        pairs: usize,
        #[arg(long, default_value_t = 3000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run the DSR and PBO gates on matched strong-lookahead and causal grids over identical zero-drift paths.
    Cleanpair {
        #[arg(long, default_value_t = 100)]
        pairs: usize,
        #[arg(long, default_value_t = 20000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run the sma grid, deflated Sharpe and CSCV PBO gates on seeded zero-drift random walks and
    /// count how many pure-noise series pass the combined gate (DSR >= 0.95 and PBO <= 0.5).
    Nullpass {
        #[arg(long, default_value_t = 100)]
        series: usize,
        #[arg(long, default_value_t = 20000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run the sma-grid DSR and PBO gates on zero-drift random walks with a causal signal and with
    /// a signal that uses the next bar's close, and count combined passes in each arm.
    Leakpass {
        #[arg(long, default_value_t = 100)]
        series: usize,
        #[arg(long, default_value_t = 20000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run the sma-grid DSR and PBO gates on zero-drift i.i.d. and GARCH(1,1) Student-t series.
    Garchpass {
        #[arg(long, default_value_t = 100)]
        series: usize,
        #[arg(long, default_value_t = 20000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run a CPCV-style accept rule (in-sample-best sma configuration per split, accept if the 5th
    /// percentile of out-of-sample split Sharpes is above zero) on zero-drift i.i.d. and GARCH series.
    Cpcvpass {
        #[arg(long, default_value_t = 100)]
        series: usize,
        #[arg(long, default_value_t = 20000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Plant a per-bar drift in i.i.d. and GARCH(1,1) Student-t series and report gate acceptance per arm.
    Garchpower {
        #[arg(long, default_value_t = 100)]
        series: usize,
        #[arg(long, default_value_t = 20000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long)]
        drift_bps: f64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Rerun the zero-drift sma null with 4, 9, 16 and 25 configurations and count combined passes.
    Gridsweep {
        #[arg(long, default_value_t = 100)]
        series: usize,
        #[arg(long, default_value_t = 20000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Plant a per-bar drift of known size in seeded random walks and report the combined gate
    /// pass rate per drift size, with Wilson intervals.
    Powersweep {
        #[arg(long, default_value_t = 100)]
        series: usize,
        #[arg(long, default_value_t = 20000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long, value_delimiter = ',', default_value = "0,1,2,4,8")]
        drifts_bps: Vec<f64>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Plant the full look-ahead rule on a fraction of bars in zero-drift random walks and report
    /// the combined gate pass count per fraction.
    Leakfrac {
        #[arg(long, default_value_t = 100)]
        series: usize,
        #[arg(long, default_value_t = 20000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long, value_delimiter = ',', default_value = "0,0.02,0.05,0.1,0.25,1")]
        fractions: Vec<f64>,
        #[arg(long)]
        out: PathBuf,
    },
    /// White's Reality Check (stationary bootstrap) null pass count beside DSR and PBO.
    Realitycheck {
        #[arg(long, default_value_t = 100)]
        series: usize,
        #[arg(long, default_value_t = 20000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long, default_value_t = 1000)]
        bootstrap_draws: usize,
        #[arg(long, default_value_t = 24.0)]
        mean_block: f64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Combined gate pass count on zero-drift random walks per CSCV embargo length.
    Embargosweep {
        #[arg(long, default_value_t = 100)]
        series: usize,
        #[arg(long, default_value_t = 20000)]
        bars: usize,
        #[arg(long, default_value_t = 7)]
        seed: u64,
        #[arg(long, value_delimiter = ',', default_value = "0,24,96,336")]
        embargoes: Vec<usize>,
        #[arg(long)]
        out: PathBuf,
    },
}

fn report(dir: &PathBuf, readme: &PathBuf) -> anyhow::Result<()> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("run-") && n.ends_with(".json")))
        .collect();
    files.sort();
    anyhow::ensure!(!files.is_empty(), "no run-*.json in {}", dir.display());
    let mut t = String::from(
        "| study | OOS Sharpe | OOS return | OOS max DD | trades | buy-and-hold Sharpe | buy-and-hold return | IS-best Sharpe (not the headline) | DSR | PBO | verdict |\n|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for f in &files {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(f)?)?;
        let g = |path: &[&str]| -> anyhow::Result<&serde_json::Value> {
            let mut cur = &v;
            for k in path {
                cur = cur.get(*k).ok_or_else(|| anyhow::anyhow!("{} missing field {}", f.display(), path.join(".")))?;
            }
            Ok(cur)
        };
        let num = |path: &[&str]| -> anyhow::Result<f64> { g(path)?.as_f64().ok_or_else(|| anyhow::anyhow!("not a number: {}", path.join("."))) };
        let study = f.file_stem().and_then(|s| s.to_str()).unwrap_or("?").trim_start_matches("run-").to_string();
        t.push_str(&format!(
            "| {} | {:.2} | {:+.1}% | {:.1}% | {} | {:.2} | {:+.1}% | {:.2} | {:.3} | {:.3} | {} |\n",
            study,
            num(&["oos_concatenated", "sharpe_annual"])?,
            num(&["oos_concatenated", "total_return"])? * 100.0,
            num(&["oos_concatenated", "max_drawdown"])? * 100.0,
            num(&["oos_concatenated", "n_trades"])? as u64,
            num(&["oos_buy_and_hold", "sharpe_annual"])?,
            num(&["oos_buy_and_hold", "total_return"])? * 100.0,
            num(&["in_sample_best_full_sample", "sharpe_annual"])?,
            num(&["deflated_sharpe_prob"])?,
            num(&["pbo"])?,
            g(&["verdict"])?.as_str().unwrap_or("?").split(':').next().unwrap_or("?"),
        ));
    }
    t.push_str("\nOOS = every out-of-sample bar concatenated, parameters chosen only on the preceding in-sample window. DSR = deflated Sharpe ratio (probability the best configuration's Sharpe beats the expected maximum of unskilled trials). PBO = probability of backtest overfitting by CSCV. Full per-window detail in `results/run-*.json`.\n");
    const START: &str = "<!-- RESULTS:START -->";
    const END: &str = "<!-- RESULTS:END -->";
    let text = std::fs::read_to_string(readme)?;
    let s = text.find(START).ok_or_else(|| anyhow::anyhow!("README missing {START}"))?;
    let e = text.find(END).ok_or_else(|| anyhow::anyhow!("README missing {END}"))?;
    anyhow::ensure!(s < e, "END before START");
    let mut out = String::new();
    out.push_str(&text[..s + START.len()]);
    out.push('\n');
    out.push_str(&t);
    out.push_str(&text[e..]);
    std::fs::write(readme, out)?;
    print!("{t}");
    Ok(())
}

// ---------- data ----------

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Bar {
    pub ts_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

fn read_bars_csv(path: &PathBuf) -> anyhow::Result<Vec<Bar>> {
    let text = std::fs::read_to_string(path)?;
    let mut bars = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 && line.starts_with("ts_ms") {
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 6 {
            continue;
        }
        bars.push(Bar {
            ts_ms: f[0].parse()?,
            open: f[1].parse()?,
            high: f[2].parse()?,
            low: f[3].parse()?,
            close: f[4].parse()?,
            volume: f[5].parse()?,
        });
    }
    anyhow::ensure!(bars.windows(2).all(|w| w[0].ts_ms < w[1].ts_ms), "bars not strictly increasing in time");
    Ok(bars)
}

fn ingest(dir: &PathBuf, out: &PathBuf) -> anyhow::Result<()> {
    let mut bars: Vec<Bar> = Vec::new();
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("zip"))
        .collect();
    names.sort();
    for p in &names {
        let f = std::fs::File::open(p)?;
        let mut z = zip::ZipArchive::new(f)?;
        for i in 0..z.len() {
            let mut entry = z.by_index(i)?;
            if !entry.name().ends_with(".csv") {
                continue;
            }
            let mut s = String::new();
            entry.read_to_string(&mut s)?;
            for line in s.lines() {
                let f: Vec<&str> = line.split(',').collect();
                if f.len() < 6 || f[0].starts_with("open_time") {
                    continue;
                }
                // Binance switched open_time from ms to us in 2025; normalise to ms.
                let mut ts: i64 = f[0].parse()?;
                if ts > 10_000_000_000_000 {
                    ts /= 1000;
                }
                bars.push(Bar {
                    ts_ms: ts,
                    open: f[1].parse()?,
                    high: f[2].parse()?,
                    low: f[3].parse()?,
                    close: f[4].parse()?,
                    volume: f[5].parse()?,
                });
            }
        }
    }
    bars.sort_by_key(|b| b.ts_ms);
    bars.dedup_by_key(|b| b.ts_ms);
    let mut w = String::from("ts_ms,open,high,low,close,volume\n");
    for b in &bars {
        w.push_str(&format!("{},{},{},{},{},{}\n", b.ts_ms, b.open, b.high, b.low, b.close, b.volume));
    }
    std::fs::write(out, w)?;
    println!("wrote {} bars from {} files to {}", bars.len(), names.len(), out.display());
    Ok(())
}

// ---------- strategies ----------

pub type Params = BTreeMap<String, f64>;

/// Target position for the NEXT bar, in [-1, 1] (long/flat only here: 0 or 1).
pub trait Strategy {
    fn target(&self, bars: &[Bar], t: usize, p: &Params) -> f64;
    fn warmup(&self, p: &Params) -> usize;
}

pub struct SmaCross;
impl Strategy for SmaCross {
    fn target(&self, bars: &[Bar], t: usize, p: &Params) -> f64 {
        let fast = p["fast"] as usize;
        let slow = p["slow"] as usize;
        if fast >= slow || t + 1 < slow {
            return 0.0;
        }
        let mean = |n: usize| bars[t + 1 - n..=t].iter().map(|b| b.close).sum::<f64>() / n as f64;
        if mean(fast) > mean(slow) {
            1.0
        } else {
            0.0
        }
    }
    fn warmup(&self, p: &Params) -> usize {
        p["slow"] as usize
    }
}

pub struct Donchian;
impl Strategy for Donchian {
    fn target(&self, bars: &[Bar], t: usize, p: &Params) -> f64 {
        let n = p["n"] as usize;
        if t < n {
            return 0.0;
        }
        let hi = bars[t - n..t].iter().map(|b| b.high).fold(f64::MIN, f64::max);
        let lo = bars[t - n..t].iter().map(|b| b.low).fold(f64::MAX, f64::min);
        let c = bars[t].close;
        if c > hi {
            1.0
        } else if c < lo {
            0.0
        } else {
            // hold previous: encoded by caller via position persistence
            f64::NAN
        }
    }
    fn warmup(&self, p: &Params) -> usize {
        p["n"] as usize + 1
    }
}

fn strategy_by_name(name: &str) -> anyhow::Result<Box<dyn Strategy>> {
    match name {
        "sma" => Ok(Box::new(SmaCross)),
        "donchian" => Ok(Box::new(Donchian)),
        other => anyhow::bail!("unknown strategy `{other}` (sma | donchian)"),
    }
}

// ---------- engine ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Costs {
    pub fee_bps: f64,
    pub slippage_bps: f64,
}

/// Per-bar simple returns of the strategy over bars[start..end), signal on bar t
/// executed at open of t+1, mark-to-market at close. Position persists when the
/// strategy returns NaN.
pub fn simulate(strat: &dyn Strategy, bars: &[Bar], p: &Params, costs: &Costs, start: usize, end: usize) -> (Vec<f64>, usize) {
    let mut rets = Vec::with_capacity(end.saturating_sub(start));
    let mut pos = 0.0f64;
    let mut trades = 0usize;
    let cost_rate = (costs.fee_bps + costs.slippage_bps) / 10_000.0;
    let mut pending: Option<f64> = None;
    for t in start..end {
        let b = bars[t];
        let mut r = 0.0;
        // Execute the target decided on the previous bar at this bar's open.
        if let Some(target) = pending.take() {
            if (target - pos).abs() > 1e-12 {
                // From open to close with the new position; cost on the traded notional.
                r += pos * (b.open / bars[t - 1].close - 1.0); // overnight gap at the old position
                r -= (target - pos).abs() * cost_rate;
                pos = target;
                r += pos * (b.close / b.open - 1.0);
                trades += 1;
            } else {
                r += pos * (b.close / bars[t - 1].close - 1.0);
            }
        } else if t > start {
            r += pos * (b.close / bars[t - 1].close - 1.0);
        }
        rets.push(r);
        let tgt = strat.target(bars, t, p);
        pending = Some(if tgt.is_nan() { pos } else { tgt });
    }
    (rets, trades)
}

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub n_bars: usize,
    pub sharpe_annual: f64,
    pub total_return: f64,
    pub max_drawdown: f64,
    pub n_trades: usize,
}

pub fn metrics(rets: &[f64], trades: usize, bars_per_year: f64) -> Metrics {
    let n = rets.len();
    let mean = rets.iter().sum::<f64>() / n.max(1) as f64;
    let var = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n.max(2) - 1) as f64;
    let sd = var.sqrt();
    let sharpe = if sd > 0.0 { mean / sd * bars_per_year.sqrt() } else { 0.0 };
    let mut eq: f64 = 1.0;
    let mut peak: f64 = 1.0;
    let mut mdd: f64 = 0.0;
    for r in rets {
        eq *= 1.0 + r;
        peak = peak.max(eq);
        mdd = mdd.max(1.0 - eq / peak);
    }
    Metrics { n_bars: n, sharpe_annual: sharpe, total_return: eq - 1.0, max_drawdown: mdd, n_trades: trades }
}

fn sharpe_per_bar(rets: &[f64]) -> f64 {
    let n = rets.len();
    if n < 2 {
        return 0.0;
    }
    let mean = rets.iter().sum::<f64>() / n as f64;
    let var = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    if var > 0.0 {
        mean / var.sqrt()
    } else {
        0.0
    }
}

// ---------- overfitting diagnostics ----------

fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

fn erf(x: f64) -> f64 {
    // Abramowitz & Stegun 7.1.26, |error| < 1.5e-7.
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * (-x * x).exp();
    if x >= 0.0 {
        y
    } else {
        -y
    }
}

fn normal_ppf(p: f64) -> f64 {
    // Acklam's rational approximation.
    let a = [-3.969683028665376e+01, 2.209460984245205e+02, -2.759285104469687e+02, 1.383577518672690e+02, -3.066479806614716e+01, 2.506628277459239e+00];
    let b = [-5.447609879822406e+01, 1.615858368580409e+02, -1.556989798598866e+02, 6.680131188771972e+01, -1.328068155288572e+01];
    let c = [-7.784894002430293e-03, -3.223964580411365e-01, -2.400758277161838e+00, -2.549732539343734e+00, 4.374664141464968e+00, 2.938163982698783e+00];
    let d = [7.784695709041462e-03, 3.224671290700398e-01, 2.445134137142996e+00, 3.754408661907416e+00];
    let plow = 0.02425;
    if p < plow {
        let q = (-2.0 * p.ln()).sqrt();
        (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5]) / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    } else if p <= 1.0 - plow {
        let q = p - 0.5;
        let r = q * q;
        (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5]) / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    }
}

/// Deflated Sharpe ratio: probability that the observed per-bar Sharpe of the
/// selected configuration exceeds the expected maximum Sharpe of `n_trials`
/// unskilled trials, given the variance of Sharpe across trials and the
/// skew/kurtosis of the selected configuration's returns.
pub fn deflated_sharpe(sr_obs: f64, sr_trials: &[f64], rets: &[f64]) -> f64 {
    let n_trials = sr_trials.len().max(2) as f64;
    let mean_sr = sr_trials.iter().sum::<f64>() / n_trials;
    let var_sr = sr_trials.iter().map(|s| (s - mean_sr).powi(2)).sum::<f64>() / (n_trials - 1.0);
    let euler = 0.5772156649015329;
    let sr0 = var_sr.sqrt() * ((1.0 - euler) * normal_ppf(1.0 - 1.0 / n_trials) + euler * normal_ppf(1.0 - 1.0 / (n_trials * std::f64::consts::E)));
    let t = rets.len() as f64;
    let mean = rets.iter().sum::<f64>() / t.max(1.0);
    let sd = (rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / t.max(1.0)).sqrt();
    let (skew, kurt) = if sd > 0.0 {
        (
            rets.iter().map(|r| ((r - mean) / sd).powi(3)).sum::<f64>() / t,
            rets.iter().map(|r| ((r - mean) / sd).powi(4)).sum::<f64>() / t,
        )
    } else {
        (0.0, 3.0)
    };
    let denom = (1.0 - skew * sr_obs + (kurt - 1.0) / 4.0 * sr_obs * sr_obs).max(1e-12).sqrt();
    normal_cdf((sr_obs - sr0) * (t - 1.0).sqrt() / denom)
}

/// Probability of backtest overfitting by CSCV. `m` is bars x configs of per-bar returns.
/// Returns (pbo, n_combinations).
pub fn pbo_cscv(m: &[Vec<f64>], n_configs: usize, s: usize) -> (f64, usize) {
    pbo_cscv_embargo(m, n_configs, s, 0)
}

/// CSCV PBO that drops `embargo` bars at both ends of every block, so no in-sample and
/// out-of-sample bar sit within `embargo` bars of a block boundary. Embargo 0 is `pbo_cscv`.
pub fn pbo_cscv_embargo(m: &[Vec<f64>], n_configs: usize, s: usize, embargo: usize) -> (f64, usize) {
    let t = m.len();
    let block = t / s;
    if block == 0 || s < 2 || n_configs < 2 || 2 * embargo >= block {
        return (f64::NAN, 0);
    }
    let blocks: Vec<usize> = (0..s).collect();
    let mut logits = Vec::new();
    for combo in combinations(&blocks, s / 2) {
        let in_set: std::collections::HashSet<usize> = combo.iter().copied().collect();
        let mut sr_is = vec![Vec::new(); n_configs];
        let mut sr_oos = vec![Vec::new(); n_configs];
        for (bi, row_start) in (0..s).map(|b| (b, b * block)) {
            let rows = &m[row_start + embargo..row_start + block - embargo];
            for cfg in 0..n_configs {
                let dst = if in_set.contains(&bi) { &mut sr_is[cfg] } else { &mut sr_oos[cfg] };
                dst.extend(rows.iter().map(|r| r[cfg]));
            }
        }
        let is_sr: Vec<f64> = sr_is.iter().map(|r| sharpe_per_bar(r)).collect();
        let oos_sr: Vec<f64> = sr_oos.iter().map(|r| sharpe_per_bar(r)).collect();
        let best = (0..n_configs).max_by(|&a, &b| is_sr[a].partial_cmp(&is_sr[b]).unwrap()).unwrap();
        let rank = oos_sr.iter().filter(|&&v| v <= oos_sr[best]).count() as f64; // 1..=n
        let w = rank / (n_configs as f64 + 1.0);
        logits.push((w / (1.0 - w)).ln());
    }
    let n = logits.len();
    let pbo = logits.iter().filter(|&&l| l <= 0.0).count() as f64 / n as f64;
    (pbo, n)
}

fn combinations(items: &[usize], k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let n = items.len();
    if k > n {
        return out;
    }
    let mut idx: Vec<usize> = (0..k).collect();
    loop {
        out.push(idx.iter().map(|&i| items[i]).collect());
        let mut i = k;
        loop {
            if i == 0 {
                return out;
            }
            i -= 1;
            if idx[i] != i + n - k {
                break;
            }
        }
        idx[i] += 1;
        for j in i + 1..k {
            idx[j] = idx[j - 1] + 1;
        }
    }
}

// ---------- pre-registration ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prereg {
    pub created_unix: u64,
    pub data_path: String,
    pub data_sha256: String,
    pub n_bars: usize,
    pub strategy: String,
    pub grid: BTreeMap<String, Vec<f64>>,
    pub n_configs: usize,
    pub is_bars: usize,
    pub oos_bars: usize,
    pub costs: Costs,
    pub bars_per_year: f64,
    pub cscv_blocks: usize,
    pub seed: u64,
    pub selection_metric: String,
    pub headline: String,
    pub prereg_sha256: String,
}

fn sha256_file(p: &PathBuf) -> anyhow::Result<String> {
    use sha2::Digest;
    Ok(hex::encode(sha2::Sha256::digest(std::fs::read(p)?)))
}

fn parse_grid(s: &str) -> anyhow::Result<BTreeMap<String, Vec<f64>>> {
    let mut g = BTreeMap::new();
    for part in s.split(';') {
        let (k, v) = part.split_once('=').ok_or_else(|| anyhow::anyhow!("grid part `{part}` needs name=values"))?;
        let vals: Vec<f64> = v.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<_, _>>()?;
        anyhow::ensure!(!vals.is_empty(), "empty grid for {k}");
        g.insert(k.trim().to_string(), vals);
    }
    Ok(g)
}

fn expand_grid(g: &BTreeMap<String, Vec<f64>>) -> Vec<Params> {
    let mut out: Vec<Params> = vec![BTreeMap::new()];
    for (k, vals) in g {
        let mut next = Vec::new();
        for p in &out {
            for v in vals {
                let mut q = p.clone();
                q.insert(k.clone(), *v);
                next.push(q);
            }
        }
        out = next;
    }
    out
}

fn prereg(data: PathBuf, strategy: String, grid: String, is_bars: usize, oos_bars: usize, fee_bps: f64, slippage_bps: f64, bars_per_year: f64, cscv_blocks: usize, seed: u64, out: PathBuf) -> anyhow::Result<()> {
    strategy_by_name(&strategy)?;
    let bars = read_bars_csv(&data)?;
    let grid = parse_grid(&grid)?;
    let n_configs = expand_grid(&grid).len();
    anyhow::ensure!(is_bars + oos_bars < bars.len(), "windows longer than the data");
    let mut p = Prereg {
        created_unix: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
        data_path: data.to_string_lossy().into_owned(),
        data_sha256: sha256_file(&data)?,
        n_bars: bars.len(),
        strategy,
        grid,
        n_configs,
        is_bars,
        oos_bars,
        costs: Costs { fee_bps, slippage_bps },
        bars_per_year,
        cscv_blocks,
        seed,
        selection_metric: "in-sample annualised Sharpe, ties to the first configuration in grid order".into(),
        headline: "concatenated out-of-sample returns across all walk-forward windows".into(),
        prereg_sha256: String::new(),
    };
    use sha2::Digest;
    p.prereg_sha256 = hex::encode(sha2::Sha256::digest(serde_json::to_vec(&p)?));
    std::fs::write(&out, serde_json::to_string_pretty(&p)?)?;
    println!("locked {} ({} configurations, {} bars, data sha256 {})", out.display(), n_configs, bars.len(), &p.data_sha256[..16]);
    Ok(())
}

// ---------- run ----------

#[derive(Debug, Serialize)]
pub struct WindowResult {
    pub is_start: usize,
    pub is_end: usize,
    pub oos_start: usize,
    pub oos_end: usize,
    pub chosen: Params,
    pub is_metrics: Metrics,
    pub oos_metrics: Metrics,
}

#[derive(Debug, Serialize)]
pub struct RunReport {
    pub tool: String,
    pub prereg_sha256: String,
    pub data_sha256: String,
    pub n_windows: usize,
    pub windows: Vec<WindowResult>,
    /// The headline: every out-of-sample bar, in order, each traded with parameters chosen before it.
    pub oos_concatenated: Metrics,
    /// Buy-and-hold over the same out-of-sample bars, same costs on the single entry.
    pub oos_buy_and_hold: Metrics,
    /// Labelled and not the headline: the best single configuration over the whole sample.
    pub in_sample_best_full_sample: Metrics,
    pub in_sample_best_params: Params,
    pub deflated_sharpe_prob: f64,
    pub pbo: f64,
    pub pbo_combinations: usize,
    pub verdict: String,
}

fn run(prereg_path: PathBuf, out: PathBuf) -> anyhow::Result<()> {
    let p: Prereg = serde_json::from_str(&std::fs::read_to_string(&prereg_path)?)?;
    let data = PathBuf::from(&p.data_path);
    let now = sha256_file(&data)?;
    anyhow::ensure!(now == p.data_sha256, "data file changed since pre-registration: {} != {}", &now[..16], &p.data_sha256[..16]);
    let bars = read_bars_csv(&data)?;
    anyhow::ensure!(bars.len() == p.n_bars, "bar count changed");
    let strat = strategy_by_name(&p.strategy)?;
    let configs = expand_grid(&p.grid);
    let _rng = rand_pcg::Pcg64Mcg::seed_from_u64(p.seed);

    // Full-sample returns matrix for PBO and DSR (bars x configs).
    let warm = configs.iter().map(|c| strat.warmup(c)).max().unwrap_or(0);
    let start_all = warm + 1;
    let mut matrix: Vec<Vec<f64>> = vec![vec![0.0; configs.len()]; bars.len() - start_all];
    let mut full_sr = Vec::new();
    let mut full_metrics = Vec::new();
    for (ci, c) in configs.iter().enumerate() {
        let (r, trades) = simulate(strat.as_ref(), &bars, c, &p.costs, start_all, bars.len());
        for (t, v) in r.iter().enumerate() {
            matrix[t][ci] = *v;
        }
        full_sr.push(sharpe_per_bar(&r));
        full_metrics.push((metrics(&r, trades, p.bars_per_year), r));
    }
    let best_full = (0..configs.len()).max_by(|&a, &b| full_sr[a].partial_cmp(&full_sr[b]).unwrap()).unwrap();
    let dsr = deflated_sharpe(full_sr[best_full], &full_sr, &full_metrics[best_full].1);
    let (pbo, n_comb) = pbo_cscv(&matrix, configs.len(), p.cscv_blocks);

    // Walk-forward.
    let mut windows = Vec::new();
    let mut oos_all: Vec<f64> = Vec::new();
    let mut oos_trades = 0usize;
    let mut bh_all: Vec<f64> = Vec::new();
    let mut is_start = start_all;
    while is_start + p.is_bars + p.oos_bars <= bars.len() {
        let is_end = is_start + p.is_bars;
        let oos_end = is_end + p.oos_bars;
        let mut best = 0usize;
        let mut best_sr = f64::MIN;
        let mut best_m = None;
        for (ci, c) in configs.iter().enumerate() {
            let (r, trades) = simulate(strat.as_ref(), &bars, c, &p.costs, is_start, is_end);
            let m = metrics(&r, trades, p.bars_per_year);
            if m.sharpe_annual > best_sr {
                best_sr = m.sharpe_annual;
                best = ci;
                best_m = Some(m);
            }
        }
        let (r_oos, trades) = simulate(strat.as_ref(), &bars, &configs[best], &p.costs, is_end, oos_end);
        oos_trades += trades;
        let m_oos = metrics(&r_oos, trades, p.bars_per_year);
        for t in is_end..oos_end {
            bh_all.push(bars[t].close / bars[t - 1].close - 1.0);
        }
        oos_all.extend(r_oos);
        windows.push(WindowResult { is_start, is_end, oos_start: is_end, oos_end, chosen: configs[best].clone(), is_metrics: best_m.unwrap(), oos_metrics: m_oos });
        is_start += p.oos_bars;
    }
    anyhow::ensure!(!windows.is_empty(), "no complete walk-forward window fits the data");
    let cost_rate = (p.costs.fee_bps + p.costs.slippage_bps) / 10_000.0;
    if let Some(first) = bh_all.first_mut() {
        *first -= cost_rate;
    }
    let oos_m = metrics(&oos_all, oos_trades, p.bars_per_year);
    let bh_m = metrics(&bh_all, 1, p.bars_per_year);
    let verdict = if oos_m.sharpe_annual > bh_m.sharpe_annual && dsr >= 0.95 && pbo <= 0.5 {
        "OOS beats buy-and-hold, DSR >= 0.95, PBO <= 0.5".to_string()
    } else {
        format!(
            "not supported: oos_sharpe {:.2} vs buy_and_hold {:.2}, DSR {:.3}, PBO {:.3}",
            oos_m.sharpe_annual, bh_m.sharpe_annual, dsr, pbo
        )
    };
    let report = RunReport {
        tool: format!("walkforward {}", env!("CARGO_PKG_VERSION")),
        prereg_sha256: p.prereg_sha256.clone(),
        data_sha256: p.data_sha256.clone(),
        n_windows: windows.len(),
        windows,
        oos_concatenated: oos_m,
        oos_buy_and_hold: bh_m,
        in_sample_best_full_sample: full_metrics[best_full].0.clone(),
        in_sample_best_params: configs[best_full].clone(),
        deflated_sharpe_prob: dsr,
        pbo,
        pbo_combinations: n_comb,
        verdict,
    };
    std::fs::write(&out, serde_json::to_string_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "headline_oos": report.oos_concatenated,
        "buy_and_hold_oos": report.oos_buy_and_hold,
        "in_sample_best_full_sample (NOT the headline)": report.in_sample_best_full_sample,
        "deflated_sharpe_prob": report.deflated_sharpe_prob,
        "pbo": report.pbo,
        "verdict": report.verdict,
    }))?);
    Ok(())
}

// ---------- cleangrade: paired flawed/clean grader ----------

/// Deliberately flawed strategy: goes long when the NEXT bar closes up. Out of range it is flat.
pub struct LookaheadShift;
impl Strategy for LookaheadShift {
    fn target(&self, bars: &[Bar], t: usize, _: &Params) -> f64 {
        match bars.get(t + 1) {
            Some(n) if n.close > bars[t].close => 1.0,
            _ => 0.0,
        }
    }
    fn warmup(&self, _: &Params) -> usize {
        0
    }
}

/// Truncation-invariance check: the target at t must not change when bars after t are removed.
pub fn truncation_flags(strat: &dyn Strategy, bars: &[Bar], p: &Params, stride: usize) -> bool {
    let first = strat.warmup(p) + 1;
    (first..bars.len()).step_by(stride.max(1)).any(|t| {
        let full = strat.target(bars, t, p);
        let cut = strat.target(&bars[..=t], t, p);
        !(full == cut || (full.is_nan() && cut.is_nan()))
    })
}

fn synthetic_bars(rng: &mut rand_pcg::Pcg64, n: usize) -> Vec<Bar> {
    synthetic_bars_drift(rng, n, 0.0)
}

/// Same walk with a fixed per-bar return `drift` (a fraction, not bps) added to every bar.
fn synthetic_bars_drift(rng: &mut rand_pcg::Pcg64, n: usize, drift: f64) -> Vec<Bar> {
    use rand::Rng;
    let mut px = 100.0f64;
    (0..n)
        .map(|i| {
            let open = px;
            px *= 1.0 + (rng.random::<f64>() - 0.5) * 0.02 + drift;
            let (hi, lo) = (open.max(px) * 1.001, open.min(px) * 0.999);
            Bar { ts_ms: i as i64 * 3_600_000, open, high: hi, low: lo, close: px, volume: 1.0 }
        })
        .collect()
}

#[derive(Default, Clone, Copy)]
struct Tally {
    tp: usize,
    fnn: usize,
    fp: usize,
    tn: usize,
}
impl Tally {
    fn add(&mut self, flawed: bool, flagged: bool) {
        match (flawed, flagged) {
            (true, true) => self.tp += 1,
            (true, false) => self.fnn += 1,
            (false, true) => self.fp += 1,
            (false, false) => self.tn += 1,
        }
    }
    fn json(&self) -> serde_json::Value {
        let rate = |a: usize, b: usize| if a + b == 0 { 0.0 } else { a as f64 / (a + b) as f64 };
        serde_json::json!({
            "flaw_recall": rate(self.tp, self.fnn),
            "clean_control_false_positive_rate": rate(self.fp, self.tn),
            "true_positives": self.tp, "false_negatives": self.fnn,
            "false_positives": self.fp, "true_negatives": self.tn,
        })
    }
}

fn utc_iso(unix: u64) -> String {
    let (days, rem) = ((unix / 86_400) as i64, unix % 86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

pub fn grade(pairs: usize, n_bars: usize, seed: u64) -> (serde_json::Value, usize) {
    let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
    let (mut always, mut never, mut trunc) = (Tally::default(), Tally::default(), Tally::default());
    let clean = Donchian;
    let mut p = Params::new();
    p.insert("n".into(), 20.0);
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let mut flawed_gross = 0.0;
    let mut clean_gross = 0.0;
    for _ in 0..pairs {
        let b = synthetic_bars(&mut rng, n_bars);
        // ground truth lives here, in-run: (is_flawed, strategy)
        let copies: [(bool, &dyn Strategy); 2] = [(true, &LookaheadShift), (false, &clean)];
        for (flawed, s) in copies {
            always.add(flawed, true);
            never.add(flawed, false);
            trunc.add(flawed, truncation_flags(s, &b, &p, 7));
            let (r, _) = simulate(s, &b, &p, &costs, 0, n_bars);
            let tot: f64 = r.iter().sum();
            if flawed { flawed_gross += tot } else { clean_gross += tot }
        }
    }
    let v = serde_json::json!({
        "reviewers": { "always_flag": always.json(), "never_flag": never.json(), "truncation": trunc.json() },
        "mean_sum_return_flawed_copy": flawed_gross / pairs.max(1) as f64,
        "mean_sum_return_clean_copy": clean_gross / pairs.max(1) as f64,
    });
    (v, pairs * 2)
}

fn cleangrade(pairs: usize, bars: usize, seed: u64, out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let (grader, n_items) = grade(pairs, bars, seed);
    let doc = serde_json::json!({
        "grader": grader,
        "config": { "pairs": pairs, "bars": bars, "seed": seed, "items_graded": n_items,
                    "clean_strategy": "donchian n=20", "flawed_strategy": "lookahead one bar" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- cleangrade --pairs {pairs} --bars {bars} --seed {seed} --out {}", out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded random-walk series generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

/// Wilson score interval (z = 1.96) for k successes in n trials.
pub fn wilson(k: usize, n: usize) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let (z, n, p) = (1.96f64, n as f64, k as f64 / n as f64);
    let d = 1.0 + z * z / n;
    let c = (p + z * z / (2.0 * n)) / d;
    let h = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / d;
    ((c - h).max(0.0), (c + h).min(1.0))
}

/// DSR and PBO of the sma grid on one series, computed on the full sample as `run` does.
pub fn gate_on_series(bars: &[Bar], costs: &Costs, blocks: usize) -> (f64, f64) {
    gate_on_series_with(&SmaCross, bars, costs, blocks)
}

/// Sma crossover whose signal at t is computed on the bar t+1 close: a deliberate one-bar leak.
pub struct LeakySma;
impl Strategy for LeakySma {
    fn target(&self, bars: &[Bar], t: usize, p: &Params) -> f64 {
        SmaCross.target(bars, (t + 1).min(bars.len() - 1), p)
    }
    fn warmup(&self, p: &Params) -> usize {
        SmaCross.warmup(p)
    }
}

pub fn gate_on_series_with(strat: &dyn Strategy, bars: &[Bar], costs: &Costs, blocks: usize) -> (f64, f64) {
    gate_on_grid(strat, bars, costs, blocks, &[10.0, 20.0, 50.0], &[100.0, 200.0, 400.0])
}

/// Same gates over the full cross product of the given fast and slow windows.
pub fn gate_on_grid(strat: &dyn Strategy, bars: &[Bar], costs: &Costs, blocks: usize, fasts: &[f64], slows: &[f64]) -> (f64, f64) {
    gate_on_grid_embargo(strat, bars, costs, blocks, fasts, slows, 0)
}

pub fn gate_on_grid_embargo(strat: &dyn Strategy, bars: &[Bar], costs: &Costs, blocks: usize, fasts: &[f64], slows: &[f64], embargo: usize) -> (f64, f64) {
    let mut configs = Vec::new();
    for &f in fasts {
        for &sl in slows {
            let mut p = Params::new();
            p.insert("fast".into(), f);
            p.insert("slow".into(), sl);
            configs.push(p);
        }
    }
    let warm = configs.iter().map(|c| strat.warmup(c)).max().unwrap_or(0);
    let start = warm + 1;
    let mut matrix = vec![vec![0.0; configs.len()]; bars.len() - start];
    let mut srs = Vec::new();
    let mut rets = Vec::new();
    for (ci, c) in configs.iter().enumerate() {
        let (r, _) = simulate(strat, bars, c, costs, start, bars.len());
        for (t, v) in r.iter().enumerate() {
            matrix[t][ci] = *v;
        }
        srs.push(sharpe_per_bar(&r));
        rets.push(r);
    }
    let best = (0..configs.len()).max_by(|&a, &b| srs[a].partial_cmp(&srs[b]).unwrap()).unwrap();
    let dsr = deflated_sharpe(srs[best], &srs, &rets[best]);
    let (pbo, _) = pbo_cscv_embargo(&matrix, configs.len(), blocks, embargo);
    (dsr, pbo)
}

pub fn nullpass_count(series: usize, n_bars: usize, seed: u64, blocks: usize) -> (serde_json::Value, usize) {
    let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let (mut pass, mut dsr_pass, mut pbo_pass) = (0usize, 0usize, 0usize);
    let mut dsrs = Vec::new();
    let mut pbos = Vec::new();
    for _ in 0..series {
        let b = synthetic_bars(&mut rng, n_bars);
        let (dsr, pbo) = gate_on_series(&b, &costs, blocks);
        let (d_ok, p_ok) = (dsr >= 0.95, pbo <= 0.5);
        dsr_pass += d_ok as usize;
        pbo_pass += p_ok as usize;
        pass += (d_ok && p_ok) as usize;
        dsrs.push(dsr);
        pbos.push(pbo);
    }
    let (lo, hi) = wilson(pass, series);
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let v = serde_json::json!({
        "null_pass_count": pass,
        "null_pass_rate": pass as f64 / series.max(1) as f64,
        "wilson95_low": lo, "wilson95_high": hi,
        "dsr_only_pass_count": dsr_pass,
        "pbo_only_pass_count": pbo_pass,
        "mean_dsr": mean(&dsrs), "mean_pbo": mean(&pbos),
    });
    (v, pass)
}

fn nullpass(series: usize, bars: usize, seed: u64, out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let blocks = 8;
    let (gates, _) = nullpass_count(series, bars, seed, blocks);
    let doc = serde_json::json!({
        "gates": gates,
        "config": { "series": series, "bars": bars, "seed": seed, "cscv_blocks": blocks,
                    "grid": "sma fast=10,20,50;slow=100,200,400", "fee_bps": 10.0, "slippage_bps": 5.0,
                    "gate": "DSR >= 0.95 and PBO <= 0.5, full sample" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- nullpass --series {series} --bars {bars} --seed {seed} --out {}", out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded random-walk series generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

/// Returns (json by drift key, pass counts in input order). Each drift reuses the same seed, so
/// the noise paths are identical across effect sizes and only the planted drift differs.
pub fn powersweep_count(series: usize, n_bars: usize, seed: u64, blocks: usize, drifts_bps: &[f64]) -> (serde_json::Value, Vec<usize>) {
    use rand::SeedableRng;
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let mut map = serde_json::Map::new();
    let mut counts = Vec::new();
    for &d in drifts_bps {
        let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
        let (mut pass, mut dsr_pass, mut pbo_pass) = (0usize, 0usize, 0usize);
        for _ in 0..series {
            let b = synthetic_bars_drift(&mut rng, n_bars, d / 10_000.0);
            let (dsr, pbo) = gate_on_series(&b, &costs, blocks);
            let (d_ok, p_ok) = (dsr >= 0.95, pbo <= 0.5);
            dsr_pass += d_ok as usize;
            pbo_pass += p_ok as usize;
            pass += (d_ok && p_ok) as usize;
        }
        let (lo, hi) = wilson(pass, series);
        map.insert(
            format!("{d}bps"),
            serde_json::json!({
                "drift_bps_per_bar": d, "pass_count": pass,
                "pass_rate": pass as f64 / series.max(1) as f64,
                "wilson95_low": lo, "wilson95_high": hi,
                "dsr_only_pass_count": dsr_pass, "pbo_only_pass_count": pbo_pass,
            }),
        );
        counts.push(pass);
    }
    (serde_json::Value::Object(map), counts)
}

fn powersweep(series: usize, bars: usize, seed: u64, drifts: &[f64], out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let blocks = 8;
    let (by, counts) = powersweep_count(series, bars, seed, blocks, drifts);
    let monotone = counts.windows(2).all(|w| w[1] >= w[0]);
    let list = drifts.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(",");
    let doc = serde_json::json!({
        "gates": { "power_by_effect_size": by, "pass_count_non_decreasing_in_drift": monotone },
        "config": { "series": series, "bars": bars, "seed": seed, "cscv_blocks": blocks, "drifts_bps": drifts,
                    "grid": "sma fast=10,20,50;slow=100,200,400", "fee_bps": 10.0, "slippage_bps": 5.0,
                    "gate": "DSR >= 0.95 and PBO <= 0.5, full sample" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- powersweep --series {series} --bars {bars} --seed {seed} --drifts-bps {list} --out {}", out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded random-walk series with planted drift, generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

/// Returns json by arm and (causal, leaky) combined pass counts. Both arms see identical paths.
pub fn leakpass_count(series: usize, n_bars: usize, seed: u64, blocks: usize) -> (serde_json::Value, (usize, usize)) {
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let arms: [(&str, &dyn Strategy); 2] = [("causal", &SmaCross), ("leaky", &LeakySma)];
    let mut map = serde_json::Map::new();
    let mut counts = Vec::new();
    for (name, strat) in arms {
        let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
        let (mut pass, mut dsr_pass, mut pbo_pass) = (0usize, 0usize, 0usize);
        let (mut dsr_sum, mut pbo_sum) = (0.0, 0.0);
        for _ in 0..series {
            let b = synthetic_bars(&mut rng, n_bars);
            let (dsr, pbo) = gate_on_series_with(strat, &b, &costs, blocks);
            let (d_ok, p_ok) = (dsr >= 0.95, pbo <= 0.5);
            dsr_pass += d_ok as usize;
            pbo_pass += p_ok as usize;
            pass += (d_ok && p_ok) as usize;
            dsr_sum += dsr;
            pbo_sum += pbo;
        }
        let (lo, hi) = wilson(pass, series);
        let n = series.max(1) as f64;
        map.insert(
            name.to_string(),
            serde_json::json!({
                "combined_pass_count": pass, "combined_pass_rate": pass as f64 / n,
                "wilson95_low": lo, "wilson95_high": hi,
                "dsr_only_pass_count": dsr_pass, "pbo_only_pass_count": pbo_pass,
                "mean_dsr": dsr_sum / n, "mean_pbo": pbo_sum / n,
            }),
        );
        counts.push(pass);
    }
    (serde_json::Value::Object(map), (counts[0], counts[1]))
}

fn leakpass(series: usize, bars: usize, seed: u64, out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let blocks = 8;
    let (arms, _) = leakpass_count(series, bars, seed, blocks);
    let doc = serde_json::json!({
        "arms": arms,
        "config": { "series": series, "bars": bars, "seed": seed, "cscv_blocks": blocks,
                    "grid": "sma fast=10,20,50;slow=100,200,400", "fee_bps": 10.0, "slippage_bps": 5.0,
                    "leaky_arm": "sma signal at t computed on the bar t+1 close",
                    "gate": "DSR >= 0.95 and PBO <= 0.5, full sample" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- leakpass --series {series} --bars {bars} --seed {seed} --out {}", out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded random-walk series generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

fn std_normal(rng: &mut rand_pcg::Pcg64) -> f64 {
    use rand::Rng;
    let (u1, u2) = (1.0 - rng.random::<f64>(), rng.random::<f64>());
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// Zero-drift GARCH(1,1) with unit-variance Student-t(5) innovations. Unconditional per-bar
/// volatility matches the uniform +-1% i.i.d. walk (0.02 / sqrt(12)); alpha 0.1, beta 0.85.
fn synthetic_bars_garch(rng: &mut rand_pcg::Pcg64, n: usize) -> Vec<Bar> {
    synthetic_bars_garch_drift(rng, n, 0.0)
}

/// Same GARCH process with a fixed per-bar return `drift` (a fraction) added after the shock.
fn synthetic_bars_garch_drift(rng: &mut rand_pcg::Pcg64, n: usize, drift: f64) -> Vec<Bar> {
    let (alpha, beta, df) = (0.10f64, 0.85f64, 5usize);
    let var0 = (0.02f64 * 0.02) / 12.0;
    let omega = var0 * (1.0 - alpha - beta);
    let scale = (3.0f64 / 5.0).sqrt();
    let (mut var, mut px) = (var0, 100.0f64);
    (0..n)
        .map(|i| {
            let chi: f64 = (0..df).map(|_| std_normal(rng).powi(2)).sum();
            let t = std_normal(rng) / (chi / df as f64).sqrt() * scale;
            let r = (var.sqrt() * t).max(-0.5);
            var = omega + alpha * r * r + beta * var;
            let open = px;
            px *= 1.0 + r + drift;
            let (hi, lo) = (open.max(px) * 1.001, open.min(px) * 0.999);
            Bar { ts_ms: i as i64 * 3_600_000, open, high: hi, low: lo, close: px, volume: 1.0 }
        })
        .collect()
}

/// Returns json of arms [iid, garch] with the combined acceptance rate at a planted drift (bps per bar).
pub fn garchpower_count(series: usize, n_bars: usize, seed: u64, blocks: usize, drift_bps: f64) -> (serde_json::Value, (usize, usize)) {
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let drift = drift_bps / 10_000.0;
    let (mut arms, mut counts) = (Vec::new(), Vec::new());
    for name in ["iid", "garch"] {
        let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
        let (mut pass, mut dsr_pass, mut pbo_pass) = (0usize, 0usize, 0usize);
        for _ in 0..series {
            let b = if name == "iid" { synthetic_bars_drift(&mut rng, n_bars, drift) } else { synthetic_bars_garch_drift(&mut rng, n_bars, drift) };
            let (dsr, pbo) = gate_on_series(&b, &costs, blocks);
            let (d_ok, p_ok) = (dsr >= 0.95, pbo <= 0.5);
            dsr_pass += d_ok as usize;
            pbo_pass += p_ok as usize;
            pass += (d_ok && p_ok) as usize;
        }
        let (lo, hi) = wilson(pass, series);
        arms.push(serde_json::json!({
            "arm": name, "accepted_count": pass, "acceptance_rate": pass as f64 / series.max(1) as f64,
            "wilson95_low": lo, "wilson95_high": hi,
            "dsr_only_pass_count": dsr_pass, "pbo_only_pass_count": pbo_pass,
        }));
        counts.push(pass);
    }
    (serde_json::Value::Array(arms), (counts[0], counts[1]))
}

fn garchpower(series: usize, bars: usize, seed: u64, drift_bps: f64, out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let blocks = 8;
    let (arms, _) = garchpower_count(series, bars, seed, blocks, drift_bps);
    let doc = serde_json::json!({
        "arms": arms,
        "config": { "series": series, "bars": bars, "seed": seed, "cscv_blocks": blocks, "drift_bps_per_bar": drift_bps,
                    "grid": "sma fast=10,20,50;slow=100,200,400", "fee_bps": 10.0, "slippage_bps": 5.0,
                    "garch": "alpha 0.10, beta 0.85, Student-t df 5 unit variance, unconditional bar vol 0.02/sqrt(12)",
                    "gate": "DSR >= 0.95 and PBO <= 0.5, full sample" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- garchpower --series {series} --bars {bars} --seed {seed} --drift-bps {drift_bps} --out {}", out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded GARCH(1,1) Student-t and i.i.d. series generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

/// Returns json of arms [iid, garch] and their combined pass counts. Same seed in both arms.
pub fn garchpass_count(series: usize, n_bars: usize, seed: u64, blocks: usize) -> (serde_json::Value, (usize, usize)) {
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let mut arms = Vec::new();
    let mut counts = Vec::new();
    for name in ["iid", "garch"] {
        let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
        let (mut pass, mut dsr_pass, mut pbo_pass) = (0usize, 0usize, 0usize);
        let (mut dsr_sum, mut pbo_sum) = (0.0, 0.0);
        for _ in 0..series {
            let b = if name == "iid" { synthetic_bars(&mut rng, n_bars) } else { synthetic_bars_garch(&mut rng, n_bars) };
            let (dsr, pbo) = gate_on_series(&b, &costs, blocks);
            let (d_ok, p_ok) = (dsr >= 0.95, pbo <= 0.5);
            dsr_pass += d_ok as usize;
            pbo_pass += p_ok as usize;
            pass += (d_ok && p_ok) as usize;
            dsr_sum += dsr;
            pbo_sum += pbo;
        }
        let (lo, hi) = wilson(pass, series);
        let n = series.max(1) as f64;
        arms.push(serde_json::json!({
            "arm": name, "null_pass_count": pass, "null_pass_rate": pass as f64 / n,
            "wilson95_low": lo, "wilson95_high": hi,
            "dsr_only_pass_count": dsr_pass, "pbo_only_pass_count": pbo_pass,
            "mean_dsr": dsr_sum / n, "mean_pbo": pbo_sum / n,
        }));
        counts.push(pass);
    }
    (serde_json::Value::Array(arms), (counts[0], counts[1]))
}

fn garchpass(series: usize, bars: usize, seed: u64, out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let blocks = 8;
    let (arms, _) = garchpass_count(series, bars, seed, blocks);
    let doc = serde_json::json!({
        "arms": arms,
        "config": { "series": series, "bars": bars, "seed": seed, "cscv_blocks": blocks,
                    "grid": "sma fast=10,20,50;slow=100,200,400", "fee_bps": 10.0, "slippage_bps": 5.0,
                    "garch": "zero drift, alpha 0.10, beta 0.85, Student-t df 5 unit variance, unconditional bar vol 0.02/sqrt(12)",
                    "gate": "DSR >= 0.95 and PBO <= 0.5, full sample" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- garchpass --series {series} --bars {bars} --seed {seed} --out {}", out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded GARCH(1,1) Student-t series generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

/// Out-of-sample per-bar Sharpe of the in-sample-best configuration for every split that holds
/// out `test_blocks` of `blocks` blocks. `embargo` bars are dropped at both ends of every block.
pub fn cpcv_split_sharpes(m: &[Vec<f64>], n_configs: usize, blocks: usize, test_blocks: usize, embargo: usize) -> Vec<f64> {
    let block = m.len() / blocks;
    if block == 0 || 2 * embargo >= block || n_configs < 1 {
        return Vec::new();
    }
    let ids: Vec<usize> = (0..blocks).collect();
    combinations(&ids, test_blocks)
        .iter()
        .map(|test| {
            let mut tr = vec![Vec::new(); n_configs];
            let mut te = vec![Vec::new(); n_configs];
            for b in 0..blocks {
                let rows = &m[b * block + embargo..(b + 1) * block - embargo];
                for c in 0..n_configs {
                    let dst = if test.contains(&b) { &mut te[c] } else { &mut tr[c] };
                    dst.extend(rows.iter().map(|r| r[c]));
                }
            }
            let is_sr: Vec<f64> = tr.iter().map(|r| sharpe_per_bar(r)).collect();
            let best = (0..n_configs).max_by(|&a, &b| is_sr[a].partial_cmp(&is_sr[b]).unwrap()).unwrap();
            sharpe_per_bar(&te[best])
        })
        .collect()
}

/// Accept iff the 5th percentile (sorted index floor(0.05 * (n - 1))) of split Sharpes is above zero.
pub fn cpcv_accept(split_sharpes: &[f64]) -> bool {
    if split_sharpes.is_empty() {
        return false;
    }
    let mut v = split_sharpes.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * 0.05) as usize] > 0.0
}

fn sma_grid_matrix(bars: &[Bar], costs: &Costs) -> Vec<Vec<f64>> {
    let mut configs = Vec::new();
    for f in [10.0, 20.0, 50.0] {
        for sl in [100.0, 200.0, 400.0] {
            let mut p = Params::new();
            p.insert("fast".into(), f);
            p.insert("slow".into(), sl);
            configs.push(p);
        }
    }
    let warm = configs.iter().map(|c| SmaCross.warmup(c)).max().unwrap_or(0);
    let start = warm + 1;
    let mut matrix = vec![vec![0.0; configs.len()]; bars.len() - start];
    for (ci, c) in configs.iter().enumerate() {
        let (r, _) = simulate(&SmaCross, bars, c, costs, start, bars.len());
        for (t, v) in r.iter().enumerate() {
            matrix[t][ci] = *v;
        }
    }
    matrix
}

/// Returns json of arms [iid, garch] and their CPCV accept counts. Same seed in both arms.
pub fn cpcvpass_count(series: usize, n_bars: usize, seed: u64, blocks: usize, test_blocks: usize, embargo: usize) -> (serde_json::Value, (usize, usize)) {
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let (mut arms, mut counts) = (Vec::new(), Vec::new());
    for name in ["iid", "garch"] {
        let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
        let (mut pass, mut n_splits, mut med_sum) = (0usize, 0usize, 0.0);
        for _ in 0..series {
            let b = if name == "iid" { synthetic_bars(&mut rng, n_bars) } else { synthetic_bars_garch(&mut rng, n_bars) };
            let m = sma_grid_matrix(&b, &costs);
            let mut ss = cpcv_split_sharpes(&m, 9, blocks, test_blocks, embargo);
            n_splits = ss.len();
            pass += cpcv_accept(&ss) as usize;
            ss.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if !ss.is_empty() {
                med_sum += ss[ss.len() / 2];
            }
        }
        let (lo, hi) = wilson(pass, series);
        let n = series.max(1) as f64;
        arms.push(serde_json::json!({
            "arm": name, "null_pass_count": pass, "null_pass_rate": pass as f64 / n,
            "wilson95_low": lo, "wilson95_high": hi,
            "splits_per_series": n_splits, "mean_median_split_sharpe_per_bar": med_sum / n,
        }));
        counts.push(pass);
    }
    (serde_json::Value::Array(arms), (counts[0], counts[1]))
}

fn cpcvpass(series: usize, bars: usize, seed: u64, out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let (blocks, test_blocks, embargo) = (8, 2, 24);
    let (arms, _) = cpcvpass_count(series, bars, seed, blocks, test_blocks, embargo);
    let doc = serde_json::json!({
        "arms": arms,
        "config": { "series": series, "bars": bars, "seed": seed, "blocks": blocks, "test_blocks": test_blocks, "embargo_bars": embargo,
                    "grid": "sma fast=10,20,50;slow=100,200,400", "fee_bps": 10.0, "slippage_bps": 5.0,
                    "garch": "zero drift, alpha 0.10, beta 0.85, Student-t df 5 unit variance, unconditional bar vol 0.02/sqrt(12)",
                    "accept_rule": "5th percentile (sorted index floor(0.05*(n-1))) of out-of-sample split Sharpes of the in-sample-best configuration > 0; embargo bars dropped at both ends of every block" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- cpcvpass --series {series} --bars {bars} --seed {seed} --out {}", out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded GARCH(1,1) Student-t and i.i.d. series generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

/// Strong full-lookahead grid: long when the close `fast/10` bars ahead beats this close by `slow/100` bps.
pub struct LookaheadGrid;
impl Strategy for LookaheadGrid {
    fn target(&self, bars: &[Bar], t: usize, p: &Params) -> f64 {
        let k = ((p["fast"] / 10.0) as usize).max(1);
        let thr = 1.0 + p["slow"] / 100.0 * 1e-4;
        match bars.get(t + k) {
            Some(n) if n.close > bars[t].close * thr => 1.0,
            _ => 0.0,
        }
    }
    fn warmup(&self, _: &Params) -> usize {
        0
    }
}

/// Matched pairs on identical zero-drift paths: (flawed combined passes, clean combined passes).
pub fn cleanpair_count(pairs: usize, n_bars: usize, seed: u64, blocks: usize) -> (serde_json::Value, (usize, usize)) {
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
    let (mut flawed, mut clean) = (0usize, 0usize);
    let (mut fd, mut fp, mut cd, mut cp) = (0usize, 0usize, 0usize, 0usize);
    for _ in 0..pairs {
        let b = synthetic_bars(&mut rng, n_bars);
        let (d1, p1) = gate_on_series_with(&LookaheadGrid, &b, &costs, blocks);
        let (d2, p2) = gate_on_series_with(&SmaCross, &b, &costs, blocks);
        let (a, bb) = (d1 >= 0.95 && p1 <= 0.5, d2 >= 0.95 && p2 <= 0.5);
        flawed += a as usize;
        clean += bb as usize;
        fd += (d1 >= 0.95) as usize;
        fp += (p1 <= 0.5) as usize;
        cd += (d2 >= 0.95) as usize;
        cp += (p2 <= 0.5) as usize;
    }
    let (fl, fh) = wilson(flawed, pairs);
    let (cl, ch) = wilson(clean, pairs);
    let n = pairs.max(1) as f64;
    let v = serde_json::json!([{
        "pair_count": pairs,
        "flawed_combined_pass_count": flawed, "flaw_recall": flawed as f64 / n,
        "flawed_wilson95_low": fl, "flawed_wilson95_high": fh,
        "flawed_dsr_only_pass_count": fd, "flawed_pbo_only_pass_count": fp,
        "clean_false_positive_count": clean, "clean_false_positive_rate": clean as f64 / n,
        "clean_wilson95_low": cl, "clean_wilson95_high": ch,
        "clean_dsr_only_pass_count": cd, "clean_pbo_only_pass_count": cp,
    }]);
    (v, (flawed, clean))
}

fn cleanpair(pairs: usize, bars: usize, seed: u64, out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let blocks = 8;
    let (arr, _) = cleanpair_count(pairs, bars, seed, blocks);
    let doc = serde_json::json!({
        "pairs": arr,
        "config": { "pairs": pairs, "bars": bars, "seed": seed, "cscv_blocks": blocks,
                    "flawed_arm": "lookahead grid: horizon fast/10 bars, threshold slow/100 bps, fast=10,20,50 slow=100,200,400",
                    "clean_arm": "sma fast=10,20,50;slow=100,200,400", "fee_bps": 10.0, "slippage_bps": 5.0,
                    "gate": "DSR >= 0.95 and PBO <= 0.5, full sample" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- cleanpair --pairs {pairs} --bars {bars} --seed {seed} --out {}", out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded random-walk series generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

/// Full look-ahead rule on bars where `mask[t]` is set, the causal sma rule elsewhere.
pub struct MaskedLookahead {
    pub mask: Vec<bool>,
}
impl Strategy for MaskedLookahead {
    fn target(&self, bars: &[Bar], t: usize, p: &Params) -> f64 {
        if self.mask.get(t).copied().unwrap_or(false) {
            LookaheadGrid.target(bars, t, p)
        } else {
            SmaCross.target(bars, t, p)
        }
    }
    fn warmup(&self, p: &Params) -> usize {
        SmaCross.warmup(p)
    }
}

/// Returns json array per leak fraction and the combined pass counts. Paths are identical across
/// fractions and the per-bar draws are shared, so the leaked bar sets are nested as fraction grows.
pub fn leakfrac_count(series: usize, n_bars: usize, seed: u64, blocks: usize, fractions: &[f64]) -> (serde_json::Value, Vec<usize>) {
    use rand::Rng;
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let mut arr = Vec::new();
    let mut counts = Vec::new();
    for &f in fractions {
        let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
        let mut mrng = rand_pcg::Pcg64::seed_from_u64(seed ^ 0x6c65_616b);
        let (mut pass, mut dsr_pass, mut pbo_pass) = (0usize, 0usize, 0usize);
        let mut leaked = 0usize;
        for _ in 0..series {
            let b = synthetic_bars(&mut rng, n_bars);
            let mask: Vec<bool> = (0..n_bars).map(|_| mrng.gen::<f64>() < f).collect();
            leaked += mask.iter().filter(|&&m| m).count();
            let (dsr, pbo) = gate_on_series_with(&MaskedLookahead { mask }, &b, &costs, blocks);
            let (d_ok, p_ok) = (dsr >= 0.95, pbo <= 0.5);
            dsr_pass += d_ok as usize;
            pbo_pass += p_ok as usize;
            pass += (d_ok && p_ok) as usize;
        }
        let (lo, hi) = wilson(pass, series);
        arr.push(serde_json::json!({
            "leak_fraction": f, "realised_leak_fraction": leaked as f64 / (series * n_bars).max(1) as f64,
            "combined_pass_count": pass, "combined_pass_rate": pass as f64 / series.max(1) as f64,
            "wilson95_low": lo, "wilson95_high": hi,
            "dsr_only_pass_count": dsr_pass, "pbo_only_pass_count": pbo_pass,
        }));
        counts.push(pass);
    }
    (serde_json::Value::Array(arr), counts)
}

fn leakfrac(series: usize, bars: usize, seed: u64, fractions: &[f64], out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let blocks = 8;
    let (arr, _) = leakfrac_count(series, bars, seed, blocks, fractions);
    let fr: Vec<String> = fractions.iter().map(|f| f.to_string()).collect();
    let doc = serde_json::json!({
        "fractions": arr,
        "config": { "series": series, "bars": bars, "seed": seed, "cscv_blocks": blocks,
                    "grid": "fast=10,20,50;slow=100,200,400", "fee_bps": 10.0, "slippage_bps": 5.0,
                    "leak_rule": "lookahead grid (horizon fast/10 bars, threshold slow/100 bps) on bars drawn with probability equal to the fraction, causal sma elsewhere",
                    "gate": "DSR >= 0.95 and PBO <= 0.5, full sample" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- leakfrac --series {series} --bars {bars} --seed {seed} --fractions {} --out {}", fr.join(","), out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded random-walk series generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

/// Per-embargo combined pass counts on identical zero-drift paths. The sma grid is simulated once
/// per series; only the CSCV step changes with the embargo.
pub fn embargosweep_count(series: usize, n_bars: usize, seed: u64, blocks: usize, embargoes: &[usize]) -> (serde_json::Value, Vec<usize>) {
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let mut arr = Vec::new();
    let mut counts = Vec::new();
    for &e in embargoes {
        let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
        let (mut pass, mut dsr_pass, mut pbo_pass) = (0usize, 0usize, 0usize);
        for _ in 0..series {
            let b = synthetic_bars(&mut rng, n_bars);
            let (dsr, pbo) = gate_on_grid_embargo(&SmaCross, &b, &costs, blocks, &[10.0, 20.0, 50.0], &[100.0, 200.0, 400.0], e);
            let (d_ok, p_ok) = (dsr >= 0.95, pbo <= 0.5);
            dsr_pass += d_ok as usize;
            pbo_pass += p_ok as usize;
            pass += (d_ok && p_ok) as usize;
        }
        let (lo, hi) = wilson(pass, series);
        arr.push(serde_json::json!({
            "embargo_bars": e, "null_pass_count": pass, "null_pass_rate": pass as f64 / series.max(1) as f64,
            "wilson95_low": lo, "wilson95_high": hi,
            "dsr_only_pass_count": dsr_pass, "pbo_only_pass_count": pbo_pass,
        }));
        counts.push(pass);
    }
    (serde_json::Value::Array(arr), counts)
}

fn embargosweep(series: usize, bars: usize, seed: u64, embargoes: &[usize], out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let blocks = 8;
    let (arr, _) = embargosweep_count(series, bars, seed, blocks, embargoes);
    let em: Vec<String> = embargoes.iter().map(|e| e.to_string()).collect();
    let doc = serde_json::json!({
        "embargo_sweep": arr,
        "config": { "series": series, "bars": bars, "seed": seed, "cscv_blocks": blocks,
                    "grid": "fast=10,20,50;slow=100,200,400", "fee_bps": 10.0, "slippage_bps": 5.0,
                    "embargo": "bars dropped at both ends of every CSCV block, in and out of sample",
                    "gate": "DSR >= 0.95 and PBO <= 0.5, full sample" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- embargosweep --series {series} --bars {bars} --seed {seed} --embargoes {} --out {}", em.join(","), out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded random-walk series generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

const GRID_FASTS: [f64; 5] = [10.0, 20.0, 35.0, 50.0, 70.0];
const GRID_SLOWS: [f64; 5] = [100.0, 150.0, 200.0, 300.0, 400.0];

/// Returns json array of per-size results and the combined pass counts. Grid side k uses the first
/// k fast and first k slow windows; every size sees identical zero-drift paths.
pub fn gridsweep_count(series: usize, n_bars: usize, seed: u64, blocks: usize, sides: &[usize]) -> (serde_json::Value, Vec<usize>) {
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let mut arr = Vec::new();
    let mut counts = Vec::new();
    for &k in sides {
        let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
        let (mut pass, mut dsr_pass, mut pbo_pass) = (0usize, 0usize, 0usize);
        for _ in 0..series {
            let b = synthetic_bars(&mut rng, n_bars);
            let (dsr, pbo) = gate_on_grid(&SmaCross, &b, &costs, blocks, &GRID_FASTS[..k], &GRID_SLOWS[..k]);
            let (d_ok, p_ok) = (dsr >= 0.95, pbo <= 0.5);
            dsr_pass += d_ok as usize;
            pbo_pass += p_ok as usize;
            pass += (d_ok && p_ok) as usize;
        }
        let (lo, hi) = wilson(pass, series);
        arr.push(serde_json::json!({
            "configs": k * k, "fast": &GRID_FASTS[..k], "slow": &GRID_SLOWS[..k],
            "combined_pass_count": pass, "combined_pass_rate": pass as f64 / series.max(1) as f64,
            "wilson95_low": lo, "wilson95_high": hi,
            "dsr_only_pass_count": dsr_pass, "pbo_only_pass_count": pbo_pass,
        }));
        counts.push(pass);
    }
    (serde_json::Value::Array(arr), counts)
}

fn gridsweep(series: usize, bars: usize, seed: u64, out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let blocks = 8;
    let (sizes, _) = gridsweep_count(series, bars, seed, blocks, &[2, 3, 4, 5]);
    let doc = serde_json::json!({
        "sizes": sizes,
        "config": { "series": series, "bars": bars, "seed": seed, "cscv_blocks": blocks,
                    "strategy": "sma", "fee_bps": 10.0, "slippage_bps": 5.0,
                    "trial_count": "equal to the number of configurations searched",
                    "gate": "DSR >= 0.95 and PBO <= 0.5, full sample" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- gridsweep --series {series} --bars {bars} --seed {seed} --out {}", out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded random-walk series generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

/// White's Reality Check p-value against a zero benchmark: statistic max_k sqrt(n) mean_k, null
/// distribution from a centred stationary bootstrap (Politis and Romano) with geometric blocks.
pub fn reality_check_p(rets: &[Vec<f64>], draws: usize, mean_block: f64, rng: &mut rand_pcg::Pcg64) -> f64 {
    use rand::Rng;
    let n = rets[0].len();
    let means: Vec<f64> = rets.iter().map(|r| r.iter().sum::<f64>() / n as f64).collect();
    let v = means.iter().cloned().fold(f64::NEG_INFINITY, f64::max) * (n as f64).sqrt();
    let q = 1.0 / mean_block.max(1.0);
    let mut idx = vec![0usize; n];
    let mut ge = 0usize;
    for _ in 0..draws {
        let mut cur = rng.random_range(0..n);
        for slot in idx.iter_mut() {
            if rng.random::<f64>() < q {
                cur = rng.random_range(0..n);
            } else {
                cur = (cur + 1) % n;
            }
            *slot = cur;
        }
        let mut vstar = f64::NEG_INFINITY;
        for (ci, r) in rets.iter().enumerate() {
            let m = idx.iter().map(|&i| r[i]).sum::<f64>() / n as f64;
            vstar = vstar.max(m - means[ci]);
        }
        if vstar * (n as f64).sqrt() >= v {
            ge += 1;
        }
    }
    (ge as f64 + 1.0) / (draws as f64 + 1.0)
}

/// Returns json and (reality-check-only, combined-with-DSR-and-PBO) pass counts on zero-drift paths.
pub fn realitycheck_count(series: usize, n_bars: usize, seed: u64, blocks: usize, draws: usize, mean_block: f64) -> (serde_json::Value, (usize, usize)) {
    let costs = Costs { fee_bps: 10.0, slippage_bps: 5.0 };
    let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
    let mut brng = rand_pcg::Pcg64::seed_from_u64(seed ^ 0x7263_6865);
    let (mut rc_pass, mut dsr_pass, mut pbo_pass, mut all_pass, mut rc_dsr, mut rc_pbo) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut p_sum = 0.0;
    for _ in 0..series {
        let bars = synthetic_bars(&mut rng, n_bars);
        let mut configs = Vec::new();
        for f in [10.0, 20.0, 50.0] {
            for sl in [100.0, 200.0, 400.0] {
                let mut p = Params::new();
                p.insert("fast".into(), f);
                p.insert("slow".into(), sl);
                configs.push(p);
            }
        }
        let start = configs.iter().map(|c| SmaCross.warmup(c)).max().unwrap_or(0) + 1;
        let mut matrix = vec![vec![0.0; configs.len()]; bars.len() - start];
        let (mut srs, mut rets) = (Vec::new(), Vec::new());
        for (ci, c) in configs.iter().enumerate() {
            let (r, _) = simulate(&SmaCross, &bars, c, &costs, start, bars.len());
            for (t, v) in r.iter().enumerate() {
                matrix[t][ci] = *v;
            }
            srs.push(sharpe_per_bar(&r));
            rets.push(r);
        }
        let best = (0..configs.len()).max_by(|&a, &b| srs[a].partial_cmp(&srs[b]).unwrap()).unwrap();
        let dsr = deflated_sharpe(srs[best], &srs, &rets[best]);
        let (pbo, _) = pbo_cscv(&matrix, configs.len(), blocks);
        let p = reality_check_p(&rets, draws, mean_block, &mut brng);
        let (rc_ok, d_ok, p_ok) = (p < 0.05, dsr >= 0.95, pbo <= 0.5);
        p_sum += p;
        rc_pass += rc_ok as usize;
        dsr_pass += d_ok as usize;
        pbo_pass += p_ok as usize;
        rc_dsr += (rc_ok && d_ok) as usize;
        rc_pbo += (rc_ok && p_ok) as usize;
        all_pass += (rc_ok && d_ok && p_ok) as usize;
    }
    let (lo, hi) = wilson(rc_pass, series);
    let n = series.max(1) as f64;
    let doc = serde_json::json!({
        "null_pass_count": rc_pass, "null_pass_rate": rc_pass as f64 / n,
        "wilson95_low": lo, "wilson95_high": hi, "mean_p_value": p_sum / n,
        "dsr_only_pass_count": dsr_pass, "pbo_only_pass_count": pbo_pass,
        "rc_and_dsr_pass_count": rc_dsr, "rc_and_pbo_pass_count": rc_pbo,
        "rc_dsr_pbo_pass_count": all_pass,
    });
    (doc, (rc_pass, all_pass))
}

fn realitycheck(series: usize, bars: usize, seed: u64, draws: usize, mean_block: f64, out: &PathBuf) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let blocks = 8;
    let (rc, _) = realitycheck_count(series, bars, seed, blocks, draws, mean_block);
    let doc = serde_json::json!({
        "reality_check": rc,
        "config": { "series": series, "bars": bars, "seed": seed, "cscv_blocks": blocks,
                    "grid": "sma fast=10,20,50;slow=100,200,400", "fee_bps": 10.0, "slippage_bps": 5.0,
                    "bootstrap_draws": draws, "mean_block": mean_block,
                    "reality_check": "max over configs of sqrt(n) mean net return vs zero benchmark, centred stationary bootstrap, p < 0.05",
                    "gate": "DSR >= 0.95 and PBO <= 0.5, full sample" },
        "provenance": {
            "started_at": utc_iso(started),
            "seeds": [seed],
            "command": format!("cargo run --locked --release -q -- realitycheck --series {series} --bars {bars} --seed {seed} --bootstrap-draws {draws} --mean-block {mean_block} --out {}", out.display()),
            "hardware": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "wall_s": t0.elapsed().as_secs_f64().round(),
            "data_sources": [{"url": "synthetic seeded random-walk series generated in-process", "license": "none (generated)"}],
        },
    });
    std::fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Ingest { dir, out } => ingest(&dir, &out),
        Cmd::Prereg { data, strategy, grid, is_bars, oos_bars, fee_bps, slippage_bps, bars_per_year, cscv_blocks, seed, out } => {
            prereg(data, strategy, grid, is_bars, oos_bars, fee_bps, slippage_bps, bars_per_year, cscv_blocks, seed, out)
        }
        Cmd::Run { prereg, out } => run(prereg, out),
        Cmd::Report { dir, readme } => report(&dir, &readme),
        Cmd::Cleangrade { pairs, bars, seed, out } => cleangrade(pairs, bars, seed, &out),
        Cmd::Nullpass { series, bars, seed, out } => nullpass(series, bars, seed, &out),
        Cmd::Leakpass { series, bars, seed, out } => leakpass(series, bars, seed, &out),
        Cmd::Cpcvpass { series, bars, seed, out } => cpcvpass(series, bars, seed, &out),
        Cmd::Garchpass { series, bars, seed, out } => garchpass(series, bars, seed, &out),
        Cmd::Garchpower { series, bars, seed, drift_bps, out } => garchpower(series, bars, seed, drift_bps, &out),
        Cmd::Cleanpair { pairs, bars, seed, out } => cleanpair(pairs, bars, seed, &out),
        Cmd::Gridsweep { series, bars, seed, out } => gridsweep(series, bars, seed, &out),
        Cmd::Leakfrac { series, bars, seed, fractions, out } => leakfrac(series, bars, seed, &fractions, &out),
        Cmd::Embargosweep { series, bars, seed, embargoes, out } => embargosweep(series, bars, seed, &embargoes, &out),
        Cmd::Realitycheck { series, bars, seed, bootstrap_draws, mean_block, out } => realitycheck(series, bars, seed, bootstrap_draws, mean_block, &out),
        Cmd::Powersweep { series, bars, seed, drifts_bps, out } => powersweep(series, bars, seed, &drifts_bps, &out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bars(closes: &[f64]) -> Vec<Bar> {
        closes
            .iter()
            .enumerate()
            .map(|(i, &c)| Bar { ts_ms: i as i64 * 3_600_000, open: c, high: c, low: c, close: c, volume: 1.0 })
            .collect()
    }

    #[test]
    fn signal_cannot_trade_on_its_own_bar() {
        // Always-long strategy: the first bar's return must be zero because the
        // position is entered at the NEXT bar's open.
        struct AlwaysLong;
        impl Strategy for AlwaysLong {
            fn target(&self, _: &[Bar], _: usize, _: &Params) -> f64 {
                1.0
            }
            fn warmup(&self, _: &Params) -> usize {
                0
            }
        }
        let b = bars(&[100.0, 110.0, 121.0]);
        let (r, trades) = simulate(&AlwaysLong, &b, &Params::new(), &Costs { fee_bps: 0.0, slippage_bps: 0.0 }, 0, 3);
        assert_eq!(r[0], 0.0);
        assert!((r[1] - 0.0).abs() < 1e-12, "entered at bar 1 open = close, flat open->close");
        assert!((r[2] - 0.10).abs() < 1e-12);
        assert_eq!(trades, 1);
    }

    #[test]
    fn costs_are_charged_on_every_fill() {
        struct Flip;
        impl Strategy for Flip {
            fn target(&self, _: &[Bar], t: usize, _: &Params) -> f64 {
                (t % 2) as f64
            }
            fn warmup(&self, _: &Params) -> usize {
                0
            }
        }
        let b = bars(&[100.0; 6]);
        let (r, trades) = simulate(&Flip, &b, &Params::new(), &Costs { fee_bps: 10.0, slippage_bps: 0.0 }, 0, 6);
        assert_eq!(trades, 4);
        let total: f64 = r.iter().sum();
        assert!((total + 4.0 * 0.001).abs() < 1e-12, "flat prices: total return is exactly the fees, got {total}");
    }

    #[test]
    fn grid_expands_to_every_combination() {
        let g = parse_grid("fast=5,10;slow=20,50,100").unwrap();
        assert_eq!(expand_grid(&g).len(), 6);
    }

    #[test]
    fn combinations_count_matches_binomial() {
        assert_eq!(combinations(&(0..16).collect::<Vec<_>>(), 8).len(), 12870);
        assert_eq!(combinations(&[0, 1, 2, 3], 2).len(), 6);
    }

    #[test]
    fn pbo_is_high_for_pure_noise() {
        // 64 bars x 10 configs of deterministic pseudo-noise: the IS-best should
        // be OOS-median on average, so PBO should sit near 0.5, not near 0.
        let mut m = vec![vec![0.0; 10]; 64];
        let mut x: u64 = 12345;
        for row in m.iter_mut() {
            for v in row.iter_mut() {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                *v = ((x >> 33) as f64 / (1u64 << 31) as f64) - 0.5;
            }
        }
        let (pbo, n) = pbo_cscv(&m, 10, 8);
        assert_eq!(n, 70);
        assert!(pbo > 0.2 && pbo < 0.8, "pbo {pbo}");
    }

    #[test]
    fn normal_functions_round_trip() {
        for p in [0.01, 0.1, 0.5, 0.9, 0.99] {
            assert!((normal_cdf(normal_ppf(p)) - p).abs() < 1e-5);
        }
    }

    #[test]
    fn deflated_sharpe_falls_as_trials_grow() {
        let rets: Vec<f64> = (0..500).map(|i| 0.001 + 0.01 * ((i as f64) * 0.7).sin()).collect();
        let sr = sharpe_per_bar(&rets);
        let few: Vec<f64> = vec![sr, sr * 0.5];
        let many: Vec<f64> = (0..200).map(|i| sr * (i as f64 / 200.0)).collect();
        assert!(deflated_sharpe(sr, &few, &rets) > deflated_sharpe(sr, &many, &rets));
    }

    #[test]
    fn truncation_reviewer_separates_flawed_from_clean() {
        let (v, n) = grade(5, 600, 7);
        assert_eq!(n, 10);
        let r = &v["reviewers"];
        assert_eq!(r["truncation"]["flaw_recall"], 1.0);
        assert_eq!(r["truncation"]["clean_control_false_positive_rate"], 0.0);
        assert_eq!(r["always_flag"]["clean_control_false_positive_rate"], 1.0);
        assert_eq!(r["never_flag"]["flaw_recall"], 0.0);
    }

    #[test]
    fn wilson_interval_brackets_the_rate() {
        let (lo, hi) = wilson(5, 100);
        assert!(lo < 0.05 && 0.05 < hi && lo > 0.0 && hi < 0.2);
        assert_eq!(wilson(0, 0), (0.0, 1.0));
    }

    #[test]
    fn nullpass_is_deterministic_and_bounded() {
        let (a, ca) = nullpass_count(3, 1500, 7, 4);
        let (b, cb) = nullpass_count(3, 1500, 7, 4);
        assert_eq!(a, b);
        assert_eq!(ca, cb);
        assert!(ca <= 3);
    }

    #[test]
    fn powersweep_is_deterministic_and_zero_drift_matches_nullpass() {
        let (a, ca) = powersweep_count(3, 1500, 7, 4, &[0.0, 8.0]);
        let (b, _) = powersweep_count(3, 1500, 7, 4, &[0.0, 8.0]);
        assert_eq!(a, b);
        let (_, null) = nullpass_count(3, 1500, 7, 4);
        assert_eq!(ca[0], null);
        assert!(a.get("8bps").is_some());
    }

    #[test]
    fn leaky_sma_is_flagged_by_truncation_and_causal_is_not() {
        let mut rng = rand_pcg::Pcg64::seed_from_u64(7);
        let b = synthetic_bars(&mut rng, 600);
        let mut p = Params::new();
        p.insert("fast".into(), 10.0);
        p.insert("slow".into(), 100.0);
        assert!(truncation_flags(&LeakySma, &b, &p, 1));
        assert!(!truncation_flags(&SmaCross, &b, &p, 1));
    }

    #[test]
    fn leakpass_is_deterministic_and_causal_arm_matches_nullpass() {
        let (a, ca) = leakpass_count(3, 1500, 7, 4);
        let (b, _) = leakpass_count(3, 1500, 7, 4);
        assert_eq!(a, b);
        let (_, null) = nullpass_count(3, 1500, 7, 4);
        assert_eq!(ca.0, null);
        assert!(ca.1 <= 3);
    }

    #[test]
    fn garchpass_is_deterministic_and_iid_arm_matches_nullpass() {
        let (a, c) = garchpass_count(3, 1500, 7, 4);
        let (b, _) = garchpass_count(3, 1500, 7, 4);
        assert_eq!(a, b);
        let (_, null) = nullpass_count(3, 1500, 7, 4);
        assert_eq!(c.0, null);
        assert_eq!(a[1]["arm"], "garch");
        let mut rng = rand_pcg::Pcg64::seed_from_u64(7);
        let bs = synthetic_bars_garch(&mut rng, 500);
        assert!(bs.iter().all(|x| x.close.is_finite() && x.close > 0.0));
    }

    #[test]
    fn garchpower_is_deterministic_and_iid_arm_matches_powersweep() {
        let (a, c) = garchpower_count(3, 1500, 7, 4, 4.0);
        let (b, _) = garchpower_count(3, 1500, 7, 4, 4.0);
        assert_eq!(a, b);
        let (_, counts) = powersweep_count(3, 1500, 7, 4, &[4.0]);
        assert_eq!(c.0, counts[0]);
        assert_eq!(a[1]["arm"], "garch");
        assert!(a[1]["acceptance_rate"].as_f64().unwrap() <= 1.0);
    }

    #[test]
    fn cpcvpass_is_deterministic_and_has_28_splits() {
        let (a, _) = cpcvpass_count(2, 3000, 7, 8, 2, 24);
        let (b, _) = cpcvpass_count(2, 3000, 7, 8, 2, 24);
        assert_eq!(a, b);
        assert_eq!(a[0]["splits_per_series"], 28);
        assert_eq!(a[1]["arm"], "garch");
        assert!(cpcv_accept(&[1.0; 28]));
        assert!(!cpcv_accept(&[-1.0; 28]));
        assert!(!cpcv_accept(&[]));
    }

    #[test]
    fn cleanpair_is_deterministic_and_lookahead_is_flagged() {
        let (a, _) = cleanpair_count(3, 1500, 7, 4);
        let (b, c) = cleanpair_count(3, 1500, 7, 4);
        assert_eq!(a, b);
        assert!(c.1 <= 3);
        let mut rng = rand_pcg::Pcg64::seed_from_u64(7);
        let bs = synthetic_bars(&mut rng, 600);
        let mut p = Params::new();
        p.insert("fast".into(), 10.0);
        p.insert("slow".into(), 100.0);
        assert!(truncation_flags(&LookaheadGrid, &bs, &p, 1));
    }

    #[test]
    fn realitycheck_is_deterministic_and_detects_planted_drift() {
        let (a, ca) = realitycheck_count(2, 1500, 7, 4, 50, 24.0);
        let (b, _) = realitycheck_count(2, 1500, 7, 4, 50, 24.0);
        assert_eq!(a, b);
        assert!(ca.0 <= 2);
        let mut rng = rand_pcg::Pcg64::seed_from_u64(1);
        let strong = vec![(0..2000).map(|i| 0.01 + 0.001 * ((i % 7) as f64 - 3.0)).collect::<Vec<f64>>()];
        assert!(reality_check_p(&strong, 100, 24.0, &mut rng) < 0.05);
        let flat = vec![(0..2000).map(|i| 0.001 * ((i % 7) as f64 - 3.0)).collect::<Vec<f64>>()];
        assert!(reality_check_p(&flat, 100, 24.0, &mut rng) > 0.05);
    }

    #[test]
    fn leakfrac_is_deterministic_and_endpoints_match_arms() {
        let (a, ca) = leakfrac_count(3, 1500, 7, 4, &[0.0, 0.1, 1.0]);
        let (b, _) = leakfrac_count(3, 1500, 7, 4, &[0.0, 0.1, 1.0]);
        assert_eq!(a, b);
        let (_, (_, clean)) = cleanpair_count(3, 1500, 7, 4);
        let (_, (flawed, _)) = cleanpair_count(3, 1500, 7, 4);
        assert_eq!(ca[0], clean);
        assert_eq!(ca[2], flawed);
        assert_eq!(a[0]["realised_leak_fraction"], 0.0);
        assert_eq!(a[2]["realised_leak_fraction"], 1.0);
    }

    #[test]
    fn embargosweep_is_deterministic_and_zero_matches_nullpass() {
        let (a, ca) = embargosweep_count(3, 1500, 7, 4, &[0, 20]);
        let (b, _) = embargosweep_count(3, 1500, 7, 4, &[0, 20]);
        assert_eq!(a, b);
        let (_, np) = cleanpair_count(3, 1500, 7, 4);
        assert_eq!(ca[0], np.1);
        assert_eq!(a[1]["embargo_bars"], 20);
    }

    #[test]
    fn embargo_zero_equals_plain_cscv_and_oversized_is_nan() {
        let m: Vec<Vec<f64>> = (0..400).map(|i| vec![((i * 7) % 11) as f64 - 5.0, ((i * 3) % 13) as f64 - 6.0, ((i * 5) % 17) as f64 - 8.0]).collect();
        assert_eq!(pbo_cscv(&m, 3, 4), pbo_cscv_embargo(&m, 3, 4, 0));
        assert!(pbo_cscv_embargo(&m, 3, 4, 50).0.is_nan());
    }

    #[test]
    fn gridsweep_is_deterministic() {
        let (a, ca) = gridsweep_count(3, 1500, 7, 4, &[2, 3]);
        let (b, _) = gridsweep_count(3, 1500, 7, 4, &[2, 3]);
        assert_eq!(a, b);
        assert_eq!(a[1]["configs"], 9);
        assert!(ca.iter().all(|&c| c <= 3));
    }

    #[test]
    fn utc_iso_known_date() {
        assert_eq!(utc_iso(1_762_858_800), "2025-11-11T11:00:00Z");
    }
}
