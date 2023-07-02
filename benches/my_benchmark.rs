use std::time::Duration;

use audio_matcher::args::Arguments;
use clap::Parser;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn full_match(c: &mut Criterion) {
    // c.bench_function("fib 20", |b| b.iter(|| fibonacci(black_box(20))));
    let input = Arguments::parse_from([
        "",
        "res/local/small_test.mp3",
        "--snippet",
        "res/local/Interlude.mp3",
        "--no-out",
        "--dry-run",
        "-n",
    ]);
    c.bench_with_input(
        BenchmarkId::new("peaks in small_test", ""),
        &input,
        |b, args| b.iter(|| audio_matcher::run(black_box(args.clone()))),
    );
}

fn correlate_vs_bib(c: &mut Criterion) {
    let mut planner = realfft::RealFftPlanner::new();

    let mode = audio_matcher::audio_matcher::Mode::Valid;
    let data1: Vec<f32> = audio_matcher::audio_matcher::test_data(100..150);
    let data2: Vec<f32> = audio_matcher::audio_matcher::test_data(-2000..2000);
    let mut group = c.benchmark_group("correlate_vs_bib");

    group.bench_function("correlate my func", |b| {
        b.iter(|| {
            audio_matcher::audio_matcher::correlate(
                &mut planner,
                black_box(&data2),
                black_box(&data1),
                black_box(&mode),
                false,
                true,
            )
            .unwrap()
            .as_ref()
            .to_vec()
        })
    });
    group.bench_function("correlate old func", |b| {
        b.iter(|| {
            fftconvolve::fftcorrelate(
                &ndarray::Array1::from_iter(black_box(data2.clone()).into_iter()),
                &ndarray::Array1::from_iter(black_box(data1.clone()).into_iter()),
                black_box(mode.into()),
            )
            .unwrap()
            .to_vec()
        })
    });
    group.finish();
}

fn correlate_vs_conj(c: &mut Criterion) {
    let mut planner = realfft::RealFftPlanner::new();

    let mode = audio_matcher::audio_matcher::Mode::Valid;
    let data1: Vec<f32> = audio_matcher::audio_matcher::test_data(100..150);
    let data2: Vec<f32> = audio_matcher::audio_matcher::test_data(-2000..2000);

    let mut group = c.benchmark_group("correlate_vs_conj");

    group.bench_function("correlate my func + conjugate", |b| {
        b.iter(|| {
            audio_matcher::audio_matcher::correlate(
                &mut planner,
                black_box(&data2),
                black_box(&data1),
                black_box(&mode),
                false,
                true,
            )
            .unwrap()
            .as_ref()
            .to_vec()
        })
    });
    group.bench_function("correlate my func + reverse mult", |b| {
        b.iter(|| {
            audio_matcher::audio_matcher::correlate(
                &mut planner,
                black_box(&data2),
                black_box(&data1),
                black_box(&mode),
                false,
                false,
            )
            .unwrap()
            .as_ref()
            .to_vec()
        })
    });
    group.finish();
}

fn full_match_duration_vs(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare_chunk_sizes");
	group.sample_size(10);
	group.measurement_time(Duration::from_secs(60));
    let input = Arguments::parse_from([
        "",
        "res/local/small_test.mp3",
        "--snippet",
        "res/local/Interlude.mp3",
        "--no-out",
        "--dry-run",
        "-n",
    ]);
    for distance in [8, 20, 60, 120] {
        let mut input = input.clone();
        input.distance = distance;
        group.bench_with_input(
            BenchmarkId::new("peaks in small_test", distance),
            &input,
            |b, args| b.iter(|| audio_matcher::run(black_box(args.clone()))),
        );
    }

    group.finish();
}

criterion_group!(benches, full_match, correlate_vs_bib, correlate_vs_conj, full_match_duration_vs);
criterion_main!(benches);