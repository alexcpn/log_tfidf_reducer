use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use logreduce::{reduce, ReduceConfig};

fn make_synthetic_log(n_lines: usize) -> String {
    let mut lines = Vec::with_capacity(n_lines);
    for i in 0..n_lines {
        let ts = format!("2026-06-01 09:{:02}:{:02}.000", (i / 60) % 60, i % 60);
        match i % 20 {
            0 => lines.push(format!("{ts} INFO  health check ok")),
            1 => lines.push(format!(
                "{ts} INFO  request {i} processed in {r}ms",
                r = i % 100
            )),
            2 if i % 100 == 2 => lines.push(format!("{ts} WARN  memory at {}%", 70 + i % 30)),
            3 if i % 200 == 3 => lines.push(format!(
                "{ts} ERROR failed to connect to db: timeout after {t}ms",
                t = i
            )),
            _ => lines.push(format!("{ts} INFO  health check ok")),
        }
    }
    lines.join("\n")
}

fn bench_reduce(c: &mut Criterion) {
    let mut group = c.benchmark_group("reduce");
    // Fewer samples for the large case to keep wall-clock reasonable
    group.sample_size(20);

    for n in [10_000usize, 100_000, 1_000_000] {
        let log = make_synthetic_log(n);
        let log_bytes = log.len() as u64;
        group.throughput(Throughput::Bytes(log_bytes));

        group.bench_with_input(BenchmarkId::new("lines", n), &log, |b, log| {
            let config = ReduceConfig {
                budget_tokens: 2000,
                context_window: 2,
                ..Default::default()
            };
            b.iter(|| {
                let out = reduce(&config, log);
                criterion::black_box(out.kept_lines)
            });
        });
    }

    group.finish();
}

// Thread-scaling benchmark: same 1M-line log at 1, 4, 8, 16 threads
fn bench_scaling(c: &mut Criterion) {
    let log = make_synthetic_log(1_000_000);
    let log_bytes = log.len() as u64;
    let mut group = c.benchmark_group("scaling");
    group.sample_size(10);

    for threads in [1usize, 4, 8, 16] {
        group.throughput(Throughput::Bytes(log_bytes));
        group.bench_with_input(BenchmarkId::new("threads", threads), &log, |b, log| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let config = ReduceConfig {
                budget_tokens: 2000,
                context_window: 2,
                ..Default::default()
            };
            b.iter(|| {
                pool.install(|| {
                    let out = reduce(&config, log);
                    criterion::black_box(out.kept_lines)
                })
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_reduce, bench_scaling);
criterion_main!(benches);
