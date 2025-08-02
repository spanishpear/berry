use berry_core::parse::parse_lockfile;
use berry_test::load_fixture;
use clap::Parser;
use memory_stats::memory_stats;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "berry-bench")]
#[command(about = "Quick benchmarking tool for berry lockfile parser")]
struct Args {
  /// Fixture file to benchmark
  #[arg(short, long)]
  fixture: Option<String>,

  /// Benchmark all fixtures
  #[arg(short, long)]
  all: bool,

  /// Output format (json, text)
  #[arg(long, default_value = "text")]
  format: String,

  /// Number of warmup runs
  #[arg(short, long, default_value = "3")]
  warmup: usize,

  /// Number of benchmark runs
  #[arg(short, long, default_value = "10")]
  runs: usize,

  /// Show detailed timing for each run
  #[arg(short, long)]
  verbose: bool,
}

#[derive(serde::Serialize)]
struct BenchmarkResult {
  fixture: String,
  file_size: usize,
  mean_time_ms: f64,
  min_time_ms: f64,
  max_time_ms: f64,
  std_dev_ms: f64,
  runs: usize,
  heap_usage_bytes: Option<usize>,
  virtual_usage_bytes: Option<usize>,
}

#[allow(clippy::cast_precision_loss)]
fn calculate_stats(times: &[f64]) -> (f64, f64, f64, f64) {
  let mean = times.iter().sum::<f64>() / times.len() as f64;
  let variance = times.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / times.len() as f64;
  let std_dev = variance.sqrt();
  let min = times.iter().fold(f64::INFINITY, |a, &b| a.min(b));
  let max = times.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

  (mean, min, max, std_dev)
}

fn benchmark_fixture(
  fixture_name: &str,
  warmup: usize,
  runs: usize,
  verbose: bool,
) -> BenchmarkResult {
  let fixture = load_fixture(fixture_name);
  let file_size = fixture.len();

  println!("Benchmarking {fixture_name} ({file_size} bytes)...");

  // Warmup runs
  for i in 0..warmup {
    let start = Instant::now();
    let result = parse_lockfile(&fixture);
    let duration = start.elapsed();
    assert!(result.is_ok(), "Should parse {fixture_name} successfully");

    if verbose {
      println!(
        "  Warmup {}: {:.3}ms - {} packages parsed",
        i + 1,
        duration.as_secs_f64() * 1000.0,
        result.unwrap().1.entries.len()
      );
    }
  }

  // Measure heap usage with a single run
  let before = memory_stats().unwrap();
  let result = parse_lockfile(&fixture);
  let after = memory_stats().unwrap();

  let heap_usage = after.physical_mem - before.physical_mem;
  let virtual_usage = after.virtual_mem - before.virtual_mem;

  assert!(result.is_ok(), "Should parse {fixture_name} successfully");

  if verbose {
    println!("  Heap usage: {heap_usage} bytes (physical), {virtual_usage} bytes (virtual)");
  }

  // Actual benchmark runs
  let mut times = Vec::new();

  for i in 0..runs {
    let start = Instant::now();
    let result = parse_lockfile(&fixture);
    let duration = start.elapsed();
    let time_ms = duration.as_secs_f64() * 1000.0;
    times.push(time_ms);

    if verbose {
      println!("  Run {}: {:.3}ms", i + 1, time_ms);
    }

    assert!(result.is_ok(), "Should parse {fixture_name} successfully");
  }

  let (mean, min, max, std_dev) = calculate_stats(&times);

  BenchmarkResult {
    fixture: fixture_name.to_string(),
    file_size,
    mean_time_ms: mean,
    min_time_ms: min,
    max_time_ms: max,
    std_dev_ms: std_dev,
    runs,
    heap_usage_bytes: Some(heap_usage),
    virtual_usage_bytes: Some(virtual_usage),
  }
}

fn print_results(results: &[BenchmarkResult], format: &str) {
  if format == "json" {
    println!("{}", serde_json::to_string_pretty(results).unwrap());
  } else {
    println!("\nBenchmark Results:");
    println!(
      "{:<25} {:<12} {:<12} {:<12} {:<12} {:<12}",
      "Fixture", "Size (bytes)", "Mean (ms)", "Min (ms)", "Max (ms)", "Heap (bytes)"
    );
    println!("{:-<97}", "");

    for result in results {
      let heap_str = result
        .heap_usage_bytes
        .map_or_else(|| "N/A".to_string(), |h| h.to_string());

      println!(
        "{:<25} {:<12} {:<12.3} {:<12.3} {:<12.3} {:<12}",
        result.fixture,
        result.file_size,
        result.mean_time_ms,
        result.min_time_ms,
        result.max_time_ms,
        heap_str
      );
    }
  }
}

fn main() {
  let args = Args::parse();

  let fixtures = if let Some(fixture) = args.fixture {
    vec![fixture]
  } else if args.all {
    vec![
      "minimal-berry.lock".to_string(),
      "workspaces.yarn.lock".to_string(),
      "yarn4-mixed-protocol.lock".to_string(),
      "yarn4-resolution.lock".to_string(),
      "yarn4-patch.lock".to_string(),
      "auxiliary-packages.yarn.lock".to_string(),
    ]
  } else {
    // Default to a few key fixtures
    vec![
      "minimal-berry.lock".to_string(),
      "workspaces.yarn.lock".to_string(),
      "auxiliary-packages.yarn.lock".to_string(),
    ]
  };

  let mut results = Vec::new();

  for fixture in fixtures {
    let result = benchmark_fixture(&fixture, args.warmup, args.runs, args.verbose);
    results.push(result);
  }

  print_results(&results, &args.format);

  // Simple regression detection
  if results.len() > 1 {
    println!("\nPerformance Analysis:");

    // Check for obvious regressions (>50% slower than fastest)
    let fastest = results
      .iter()
      .min_by(|a, b| a.mean_time_ms.partial_cmp(&b.mean_time_ms).unwrap())
      .unwrap();

    for result in &results {
      if result.fixture != fastest.fixture {
        let ratio = result.mean_time_ms / fastest.mean_time_ms;
        if ratio > 1.5 {
          println!(
            "⚠️  {} is {:.1}x slower than {} (potential regression)",
            result.fixture, ratio, fastest.fixture
          );
        } else {
          println!(
            "✅ {} performance looks normal ({:.1}x vs fastest)",
            result.fixture, ratio
          );
        }
      }
    }
  }
}
