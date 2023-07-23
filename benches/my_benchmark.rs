use std::time::Duration;

use ::audio_matcher::matcher::{
    args::Arguments,
    audio_matcher::{self, CorrelateAlgo, Mode},
    mp3_reader,
};
use clap::Parser;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// fn full_match(c: &mut Criterion) {
//     // c.bench_function("fib 20", |b| b.iter(|| fibonacci(black_box(20))));
//     let input = Arguments::parse_from([
//         "",
//         "res/local/small_test.mp3",
//         "--snippet",
//         "res/local/Interlude.mp3",
//         "--no-out",
//         "--dry-run",
//         "-n",
//     ]);
//     c.bench_with_input(
//         BenchmarkId::new("peaks in small_test", ""),
//         &input,
//         |b, args| b.iter(|| audio_matcher::run(black_box(args.clone()))),
//     );
// }

fn correlate_vs_bib(c: &mut Criterion) {
    let mode = Mode::Valid;
    let data1: Vec<f32> = audio_matcher::test_data(100..150);
    let data2: Vec<f32> = audio_matcher::test_data(-2000..2000);

    let my_algo = audio_matcher::MyConvolve::new(data1.clone().into());
    let lib_algo = audio_matcher::LibConvolve::new(data1.into());

    let mut group = c.benchmark_group("correlate_vs_bib");
    group.bench_function("correlate my func", |b| {
        b.iter(|| {
            my_algo
                .correlate_with_sample(black_box(&data2), mode, false)
                .unwrap()
        })
    });
    group.bench_function("correlate old func", |b| {
        b.iter(|| {
            lib_algo
                .correlate_with_sample(black_box(&data2), mode, false)
                .unwrap()
        })
    });
    group.finish();
}

fn correlate_vs_conj(c: &mut Criterion) {
    let mode = Mode::Valid;
    let data1: Vec<f32> = audio_matcher::test_data(100..150);
    let data2: Vec<f32> = audio_matcher::test_data(-2000..2000);
    let mut my_algo = audio_matcher::MyConvolve::new(data1.into());

    let mut group = c.benchmark_group("correlate_vs_conj");

    group.bench_function("correlate my func + conjugate", |b| {
        b.iter(|| {
            my_algo
                .correlate_with_sample(black_box(&data2), mode, false)
                .unwrap()
        })
    });
    my_algo.use_conjugation = false;
    group.bench_function("correlate my func + reverse mult", |b| {
        b.iter(|| {
            my_algo
                .correlate_with_sample(black_box(&data2), mode, false)
                .unwrap()
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
        "--silent",
        "-n",
    ]);
    for distance in [8, 20, 60, 120] {
        let mut input = input.clone();
        input.distance = distance;
        group.bench_with_input(
            BenchmarkId::new("peaks in small_test", distance),
            &input,
            |b, args| b.iter(|| ::audio_matcher::matcher::run(black_box(args))),
        );
    }

    group.finish();
}

fn mp3_duration_vs_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_duration_vs_parallel");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(200));
    let input = "res/local/big_test.mp3";
    for parallel in [true, false] {
        group.bench_with_input(
            BenchmarkId::new("get_mp3_duration", parallel),
            &parallel,
            |b, args| {
                b.iter(|| mp3_reader::mp3_duration(black_box(&input), black_box(*args)).unwrap())
            },
        );
    }

    group.finish();
}

fn read_mp3(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_mp3");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(240));
    let input = "res/local/small_test.mp3";
    group.bench_function("read_mp3", |b| {
        b.iter(|| {
            mp3_reader::read_mp3(black_box(&input))
                .unwrap()
                .1
                .collect::<Vec<_>>()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    // full_match,
    correlate_vs_bib,
    correlate_vs_conj,
    full_match_duration_vs,
    mp3_duration_vs_parallel,
    read_mp3
);
criterion_main!(benches);
