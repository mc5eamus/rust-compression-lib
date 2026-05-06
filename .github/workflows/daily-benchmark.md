---
name: Daily Benchmark Report
description: Runs Rust compression benchmarks daily and reports results as a GitHub issue.
on:
  schedule: daily
permissions:
  contents: read
  actions: read
  issues: read
  pull-requests: read
engine: copilot
strict: true
timeout-minutes: 30
network:
  allowed: [defaults, rust]
tools:
  github:
    toolsets: [default]
  bash: ["*"]
safe-outputs:
  create-issue:
    title-prefix: "[benchmark] "
    labels: [benchmark]
---

# Daily Benchmark Report

You are running daily Rust compression benchmarks for the repository `${{ github.repository }}`.

## Your Task

1. Run the benchmarks using:
   ```
   cargo bench 2>&1
   ```
2. Collect all benchmark output, including timing data, throughput, compression ratios, and any warnings or errors.
3. Create a GitHub issue summarizing the results.

## Issue Content Guidelines

The issue should include:

- **Date**: Today's date in ISO format (YYYY-MM-DD)
- **Summary table**: For each benchmark group (`compress_by_level`, `decompress_by_level`, `parallel_compress_by_level`, `parallel_decompress_by_level`) and each compression level (1, 5, 9), include the measured time and throughput.
- **Compression ratios**: Include the compression ratio and space saving percentage for each level reported in stdout.
- **Notable observations**: Highlight any significant changes, regressions, or anomalies compared to Criterion's built-in change detection output.
- **Raw output**: Attach the full raw benchmark output in a `<details>` collapsible block for reference.

Use clear markdown formatting with headers and tables. Keep the issue concise and informative.
