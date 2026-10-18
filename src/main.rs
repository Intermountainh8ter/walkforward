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
    let t = m.len();
    let block = t / s;
    if block == 0 || s < 2 || n_configs < 2 {
        return (f64::NAN, 0);
    }
    let blocks: Vec<usize> = (0..s).collect();
    let mut logits = Vec::new();
    for combo in combinations(&blocks, s / 2) {
        let in_set: std::collections::HashSet<usize> = combo.iter().copied().collect();
        let mut sr_is = vec![Vec::new(); n_configs];
        let mut sr_oos = vec![Vec::new(); n_configs];
        for (bi, row_start) in (0..s).map(|b| (b, b * block)) {
            let rows = &m[row_start..row_start + block];
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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Ingest { dir, out } => ingest(&dir, &out),
        Cmd::Prereg { data, strategy, grid, is_bars, oos_bars, fee_bps, slippage_bps, bars_per_year, cscv_blocks, seed, out } => {
            prereg(data, strategy, grid, is_bars, oos_bars, fee_bps, slippage_bps, bars_per_year, cscv_blocks, seed, out)
        }
        Cmd::Run { prereg, out } => run(prereg, out),
        Cmd::Report { dir, readme } => report(&dir, &readme),
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
}
