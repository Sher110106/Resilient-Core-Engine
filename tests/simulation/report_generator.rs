//! Benchmark report generator
//!
//! Generates comprehensive reports in Markdown, JSON, and HTML formats

use super::metrics::{BenchmarkMetrics, BenchmarkResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Complete benchmark report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub generated_at: String,
    pub system_info: SystemInfo,
    pub summary: ReportSummary,
    pub claim_validation: ClaimValidation,
    pub performance_curves: PerformanceCurves,
    pub detailed_results: Vec<BenchmarkResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub rust_version: String,
    pub erasure_config: String,
    pub chunk_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub pass_rate: f64,
    pub total_duration_secs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimValidation {
    pub claim: String,
    pub validated: bool,
    pub success_rate_at_20_percent: f64,
    pub max_tolerable_loss: f64,
    pub details: Vec<LossRateResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LossRateResult {
    pub loss_rate: f32,
    pub tests_run: usize,
    pub tests_passed: usize,
    pub success_rate: f64,
    pub avg_throughput_mbps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceCurves {
    pub throughput_vs_loss: Vec<(f32, f64)>,
    pub throughput_vs_file_size: Vec<(usize, f64)>,
    pub latency_vs_file_size: Vec<(usize, u64)>,
}

impl BenchmarkReport {
    /// Create a new report from benchmark results
    pub fn from_results(results: Vec<BenchmarkResult>) -> Self {
        let generated_at = chrono::Utc::now().to_rfc3339();

        let total_tests = results.len();
        let passed = results.iter().filter(|r| r.is_success()).count();
        let failed = total_tests - passed;
        let pass_rate = if total_tests > 0 {
            passed as f64 / total_tests as f64 * 100.0
        } else {
            0.0
        };

        let total_duration_secs: f64 = results
            .iter()
            .map(|r| r.metrics.transfer_duration_ms as f64 / 1000.0)
            .sum();

        // Group by loss rate for claim validation
        let mut by_loss_rate: HashMap<u32, Vec<&BenchmarkResult>> = HashMap::new();
        for result in &results {
            let key = (result.metrics.packet_loss_rate * 100.0) as u32;
            by_loss_rate.entry(key).or_default().push(result);
        }

        let mut loss_rate_results: Vec<LossRateResult> = by_loss_rate
            .iter()
            .map(|(&loss_pct, tests)| {
                let passed = tests.iter().filter(|t| t.is_success()).count();
                let total = tests.len();
                let avg_throughput = tests
                    .iter()
                    .map(|t| t.metrics.throughput_mbps())
                    .sum::<f64>()
                    / total as f64;

                LossRateResult {
                    loss_rate: loss_pct as f32 / 100.0,
                    tests_run: total,
                    tests_passed: passed,
                    success_rate: passed as f64 / total as f64 * 100.0,
                    avg_throughput_mbps: avg_throughput,
                }
            })
            .collect();

        loss_rate_results.sort_by(|a, b| a.loss_rate.partial_cmp(&b.loss_rate).unwrap());

        // Find success rate at 20% loss
        let success_at_20 = loss_rate_results
            .iter()
            .find(|r| (r.loss_rate - 0.20).abs() < 0.01)
            .map(|r| r.success_rate)
            .unwrap_or(0.0);

        // Find max tolerable loss (>=90% success)
        let max_tolerable = loss_rate_results
            .iter()
            .filter(|r| r.success_rate >= 90.0)
            .map(|r| r.loss_rate)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        // Build performance curves
        let throughput_vs_loss: Vec<(f32, f64)> = loss_rate_results
            .iter()
            .map(|r| (r.loss_rate, r.avg_throughput_mbps))
            .collect();

        // Group by file size for file size curves
        let mut by_file_size: HashMap<usize, Vec<&BenchmarkResult>> = HashMap::new();
        for result in &results {
            by_file_size
                .entry(result.metrics.file_size)
                .or_default()
                .push(result);
        }

        let mut throughput_vs_file_size: Vec<(usize, f64)> = by_file_size
            .iter()
            .map(|(&size, tests)| {
                let avg = tests
                    .iter()
                    .map(|t| t.metrics.throughput_mbps())
                    .sum::<f64>()
                    / tests.len() as f64;
                (size, avg)
            })
            .collect();
        throughput_vs_file_size.sort_by_key(|(size, _)| *size);

        let mut latency_vs_file_size: Vec<(usize, u64)> = by_file_size
            .iter()
            .map(|(&size, tests)| {
                let avg = tests
                    .iter()
                    .map(|t| t.metrics.transfer_duration_ms)
                    .sum::<u64>()
                    / tests.len() as u64;
                (size, avg)
            })
            .collect();
        latency_vs_file_size.sort_by_key(|(size, _)| *size);

        Self {
            generated_at,
            system_info: SystemInfo {
                os: std::env::consts::OS.to_string(),
                rust_version: "1.75+".to_string(),
                erasure_config: "50 data + 10 parity shards".to_string(),
                chunk_size: 512 * 1024,
            },
            summary: ReportSummary {
                total_tests,
                passed,
                failed,
                pass_rate,
                total_duration_secs,
            },
            claim_validation: ClaimValidation {
                claim: "20% packet loss tolerance".to_string(),
                validated: success_at_20 >= 95.0,
                success_rate_at_20_percent: success_at_20,
                max_tolerable_loss: max_tolerable as f64 * 100.0,
                details: loss_rate_results,
            },
            performance_curves: PerformanceCurves {
                throughput_vs_loss,
                throughput_vs_file_size,
                latency_vs_file_size,
            },
            detailed_results: results,
        }
    }

    /// Generate Markdown report
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("# RESILIENT Benchmark Report\n\n");
        md.push_str(&format!("> Generated: {}\n\n", self.generated_at));

        md.push_str("---\n\n");

        // Executive Summary
        md.push_str("## Executive Summary\n\n");
        md.push_str(&format!(
            "- **Total Tests:** {}\n",
            self.summary.total_tests
        ));
        md.push_str(&format!(
            "- **Passed:** {} ({:.1}%)\n",
            self.summary.passed, self.summary.pass_rate
        ));
        md.push_str(&format!("- **Failed:** {}\n", self.summary.failed));
        md.push_str(&format!(
            "- **Total Duration:** {:.1}s\n\n",
            self.summary.total_duration_secs
        ));

        // Claim Validation
        md.push_str("## Claim Validation\n\n");
        md.push_str(&format!("### \"{}\"\n\n", self.claim_validation.claim));

        md.push_str("| Loss Rate | Tests | Passed | Success % | Avg Throughput |\n");
        md.push_str("|-----------|-------|--------|-----------|----------------|\n");

        for result in &self.claim_validation.details {
            md.push_str(&format!(
                "| {:.0}% | {} | {} | {:.1}% | {:.2} MB/s |\n",
                result.loss_rate * 100.0,
                result.tests_run,
                result.tests_passed,
                result.success_rate,
                result.avg_throughput_mbps
            ));
        }

        md.push_str("\n");

        let verdict = if self.claim_validation.validated {
            "CLAIM VALIDATED"
        } else {
            "CLAIM NOT VALIDATED"
        };
        let emoji = if self.claim_validation.validated {
            "✅"
        } else {
            "❌"
        };

        md.push_str(&format!(
            "**VERDICT: {} {}** (Success rate at 20% loss: {:.1}%)\n\n",
            emoji, verdict, self.claim_validation.success_rate_at_20_percent
        ));

        md.push_str(&format!(
            "**Maximum Tolerable Loss (≥90% success):** {:.1}%\n\n",
            self.claim_validation.max_tolerable_loss
        ));

        // Performance Curves
        md.push_str("## Performance Analysis\n\n");

        md.push_str("### Throughput vs Packet Loss\n\n");
        md.push_str("| Loss Rate | Throughput (MB/s) |\n");
        md.push_str("|-----------|-------------------|\n");
        for (loss, throughput) in &self.performance_curves.throughput_vs_loss {
            md.push_str(&format!("| {:.0}% | {:.2} |\n", loss * 100.0, throughput));
        }
        md.push_str("\n");

        md.push_str("### Throughput vs File Size\n\n");
        md.push_str("| File Size | Throughput (MB/s) |\n");
        md.push_str("|-----------|-------------------|\n");
        for (size, throughput) in &self.performance_curves.throughput_vs_file_size {
            md.push_str(&format!("| {} | {:.2} |\n", format_size(*size), throughput));
        }
        md.push_str("\n");

        // System Info
        md.push_str("## System Information\n\n");
        md.push_str(&format!("- **OS:** {}\n", self.system_info.os));
        md.push_str(&format!(
            "- **Rust Version:** {}\n",
            self.system_info.rust_version
        ));
        md.push_str(&format!(
            "- **Erasure Config:** {}\n",
            self.system_info.erasure_config
        ));
        md.push_str(&format!(
            "- **Chunk Size:** {}\n\n",
            format_size(self.system_info.chunk_size)
        ));

        md.push_str("---\n\n");
        md.push_str("*Report generated by RESILIENT Benchmark Suite*\n");

        md
    }

    /// Generate JSON report
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Save report to files
    pub fn save(&self, output_dir: &Path) -> std::io::Result<()> {
        fs::create_dir_all(output_dir)?;

        // Save Markdown
        let md_path = output_dir.join("benchmark_report.md");
        fs::write(&md_path, self.to_markdown())?;

        // Save JSON
        let json_path = output_dir.join("benchmark_report.json");
        fs::write(&json_path, self.to_json())?;

        println!("Reports saved to:");
        println!("  - {}", md_path.display());
        println!("  - {}", json_path.display());

        Ok(())
    }
}

fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{} MB", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_generation() {
        let results = vec![
            BenchmarkResult::success(
                "test_1",
                BenchmarkMetrics {
                    file_size: 1024 * 1024,
                    packet_loss_rate: 0.10,
                    latency_ms: 50,
                    bandwidth_bps: 1_000_000,
                    concurrent_transfers: 1,
                    throughput_bps: 8_000_000.0,
                    goodput_bps: 7_500_000.0,
                    transfer_duration_ms: 1000,
                    first_byte_latency_ms: 10,
                    chunks_sent: 10,
                    chunks_lost: 1,
                    chunks_recovered: 1,
                    retransmissions: 0,
                    peak_memory_bytes: 0,
                    cpu_time_ms: 0,
                    success: true,
                    checksum_valid: true,
                    bytes_corrupted: 0,
                },
            ),
            BenchmarkResult::success(
                "test_2",
                BenchmarkMetrics {
                    file_size: 1024 * 1024,
                    packet_loss_rate: 0.20,
                    latency_ms: 50,
                    bandwidth_bps: 1_000_000,
                    concurrent_transfers: 1,
                    throughput_bps: 6_000_000.0,
                    goodput_bps: 5_500_000.0,
                    transfer_duration_ms: 1500,
                    first_byte_latency_ms: 10,
                    chunks_sent: 10,
                    chunks_lost: 2,
                    chunks_recovered: 2,
                    retransmissions: 0,
                    peak_memory_bytes: 0,
                    cpu_time_ms: 0,
                    success: true,
                    checksum_valid: true,
                    bytes_corrupted: 0,
                },
            ),
        ];

        let report = BenchmarkReport::from_results(results);

        assert_eq!(report.summary.total_tests, 2);
        assert_eq!(report.summary.passed, 2);
        assert_eq!(report.summary.pass_rate, 100.0);

        let md = report.to_markdown();
        assert!(md.contains("RESILIENT Benchmark Report"));
        assert!(md.contains("Executive Summary"));
    }
}
