use audio_matcher::args::Arguments;
use clap::Parser;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn criterion_benchmark(c: &mut Criterion) {
    // c.bench_function("fib 20", |b| b.iter(|| fibonacci(black_box(20))));
	let input = Arguments::parse_from(["", "res/local/small_test.mp3", "--snippet", "res/local/Interlude.mp3", "--no-out", "--dry-run", "-n"]);
	c.bench_with_input(BenchmarkId::new("peaks in small_test", ""), &input, |b, args|{
		b.iter(||audio_matcher::run(black_box(args.clone())))
	});
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
