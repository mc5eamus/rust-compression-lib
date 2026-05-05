use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use rust_compressor::{compress_with_level, decompress, parallel_compress_with_options, parallel_decompress};
use std::fs;
use rayon;

const LEVELS: &[i32] = &[1, 5, 9];
const sample_path: &str = "sample1.fna";

fn adaptive_chunk_size(data_len: usize) -> usize {
    let target_chunks = rayon::current_num_threads().max(2) / 2;
    (data_len / target_chunks).max(1)
}

fn load_sample() -> Vec<u8> {
    let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), sample_path);
    fs::read(&path)
        .expect(&format!("failed to read {} — place it in the project root", sample_path))
}

fn bench_compress_levels(c: &mut Criterion) {
    let data = load_sample();

    // Print compression ratios for each level
    println!("\n--- Compression ratio ({}, {} bytes) ---", sample_path, data.len());
    for &level in LEVELS {
        let compressed = compress_with_level(&data, level).expect("compress failed");
        let ratio = data.len() as f64 / compressed.len() as f64;
        let saving = (1.0 - compressed.len() as f64 / data.len() as f64) * 100.0;
        println!(
            "  level {:>2}: {} -> {} bytes  (ratio {:.3}x, saving {:.1}%)",
            level,
            data.len(),
            compressed.len(),
            ratio,
            saving
        );
    }
    println!("---");

    let mut group = c.benchmark_group("compress_by_level");
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    for &level in LEVELS {
        group.bench_with_input(BenchmarkId::from_parameter(level), &data, |b, data| {
            b.iter(|| compress_with_level(data, level).expect("compress failed"));
        });
    }

    group.finish();
}

fn bench_decompress_levels(c: &mut Criterion) {
    let data = load_sample();

    let compressed: Vec<(i32, Vec<u8>)> = LEVELS
        .iter()
        .map(|&level| {
            let c = compress_with_level(&data, level).expect("pre-compress failed");
            (level, c)
        })
        .collect();

    let mut group = c.benchmark_group("decompress_by_level");
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    for (level, comp) in &compressed {
        group.bench_with_input(BenchmarkId::from_parameter(level), comp, |b, comp| {
            b.iter(|| decompress(comp).expect("decompress failed"));
        });
    }

    group.finish();
}

fn bench_parallel_compress(c: &mut Criterion) {
    let data = load_sample();
    let chunk_size = adaptive_chunk_size(data.len());
    let num_chunks = (data.len() + chunk_size - 1) / chunk_size;
    let num_threads = rayon::current_num_threads();

    println!(
        "\n--- Parallel compression ({}, {} bytes, chunk={}KB, chunks={}, threads={}) ---",
        sample_path,
        data.len(),
        chunk_size / 1024,
        num_chunks,
        num_threads
    );
    for &level in LEVELS {
        let compressed =
            parallel_compress_with_options(&data, level, chunk_size).expect("compress failed");
        let ratio = data.len() as f64 / compressed.len() as f64;
        let saving = (1.0 - compressed.len() as f64 / data.len() as f64) * 100.0;
        println!(
            "  level {:>2}: {} -> {} bytes  (ratio {:.3}x, saving {:.1}%)",
            level,
            data.len(),
            compressed.len(),
            ratio,
            saving
        );
    }
    println!("---");

    let mut group = c.benchmark_group("parallel_compress_by_level");
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    for &level in LEVELS {
        group.bench_with_input(BenchmarkId::from_parameter(level), &data, |b, data| {
            b.iter(|| {
                parallel_compress_with_options(data, level, chunk_size)
                    .expect("parallel compress failed")
            });
        });
    }

    group.finish();
}

fn bench_parallel_decompress(c: &mut Criterion) {
    let data = load_sample();
    let chunk_size = adaptive_chunk_size(data.len());

    let compressed: Vec<(i32, Vec<u8>)> = LEVELS
        .iter()
        .map(|&level| {
            let comp =
                parallel_compress_with_options(&data, level, chunk_size).expect("compress failed");
            (level, comp)
        })
        .collect();

    let mut group = c.benchmark_group("parallel_decompress_by_level");
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    for (level, comp) in &compressed {
        group.bench_with_input(BenchmarkId::from_parameter(level), comp, |b, comp| {
            b.iter(|| parallel_decompress(comp).expect("parallel decompress failed"));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_compress_levels,
    bench_decompress_levels,
    bench_parallel_compress,
    bench_parallel_decompress
);
criterion_main!(benches);
