use crate::chunked;
use crate::leveled_output::verbose;
use crate::offset_range;
use crate::progress_bar::ProgressBar;

use find_peaks::Peak;
use itertools::Itertools;
use ndarray::Array1;
use realfft::{
    num_complex::{Complex, Complex32, ComplexFloat},
    num_traits::Zero,
    FftNum, RealFftPlanner,
};

use std::{
    time::{Duration, Instant},
    vec,
};

use crate::mp3_reader::SampleType;

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub chunk_size: Duration,
    pub overlap_length: Duration,
    pub distance: Duration,
    pub prominence: SampleType,
}

pub fn calc_chunks(
    sr: u16,
    m_samples: impl Iterator<Item = SampleType> + 'static,
    s_samples: impl Iterator<Item = SampleType>,
    m_duration: Duration,
    config: Config,
) -> Vec<find_peaks::Peak<SampleType>> {
    use ndarray::Array1;

    use std::sync::{Arc, Mutex};
    use threadpool::ThreadPool;

    // normalize inputs
    let chunks = ((m_duration).as_secs_f64() / config.chunk_size.as_secs_f64()).ceil() as usize;
    let overlap_length = (config.overlap_length.as_secs_f64() * sr as f64).round() as u64;
    let chunk_size = (config.chunk_size.as_secs_f64() * sr as f64).round() as u64;

    verbose(&"collecting snippet");
    let s_samples: Arc<Array1<SampleType>> = Arc::new(Array1::from_iter(s_samples));
    verbose(&"collected snippet");

    let progress_state = Arc::new(Mutex::new((0, 0)));
    let progress_bar = Arc::new(
        ProgressBar {
            bar_length: chunks.min(80),
            ..ProgressBar::default()
        }
        .prepare_output(),
    );

    // threadpool size = Number of Available Cores * (1 + Wait time / Work time)
    // should use less, cause RAM fills up
    let n_workers = 6;
    let pool = ThreadPool::new(n_workers);

    let (tx, rx) = std::sync::mpsc::channel::<Vec<Peak<SampleType>>>();
    let start = Instant::now();

    for (i, chunk) in chunked(
        m_samples,
        chunk_size as usize + overlap_length as usize,
        chunk_size as usize,
    )
    .enumerate()
    {
        if chunks <= i {
            panic!("to many chunks")
        }
        let s_samples = Arc::clone(&s_samples);
        let progress_state = Arc::clone(&progress_state);
        let progress_bar = Arc::clone(&progress_bar);
        let tx = tx.clone();
        pool.execute(move || {
            let mut lock = progress_state.lock().unwrap();
            lock.1 += 1; // incrementing started counter
            progress_bar.print_progress(&lock, chunks, &start);
            drop(lock);

            let offset = chunk_size as usize * i;
            let m_samples = Array1::from_iter(chunk.into_iter());
            let _matches =
                fftconvolve::fftcorrelate(&m_samples, &s_samples, fftconvolve::Mode::Valid)
                    .unwrap()
                    .to_vec();
            let peaks = find_peaks(&_matches, sr, config)
                .iter()
                .map(|p| {
                    let mut p = p.clone();
                    p.position = offset_range(&p.position, offset);
                    p
                })
                .collect::<Vec<_>>();

            let mut lock = progress_state.lock().unwrap();
            lock.0 += 1; // incrementing finished counter
            progress_bar.print_progress(&lock, chunks, &start);
            drop(lock);
            drop(progress_bar);

            tx.send(peaks)
                .expect("channel will be there waiting for the pool");
        });
    }
    pool.join();
    Arc::into_inner(progress_bar)
        .expect("reference to Arc<ProgressBar> remaining")
        .finish_output();
    if pool.panic_count() > 0 {
        panic!("some worker threads paniced");
    }
    rx.iter()
        .take(chunks)
        .flatten()
        .sorted_by(|a, b| Ord::cmp(&a.position.start, &b.position.start))
        .collect_vec()
}

impl ProgressBar<2, crate::progress_bar::Open> {
    fn print_progress(&self, data: &(usize, usize), chunks: usize, start: &Instant) {
        let elapsed = Instant::now().duration_since(*start);
        let (_, minutes, seconds) = crate::split_duration(&elapsed);
        let fmt_elapsed = &format!(" {:0>2}:{:0>2}", minutes, seconds);

        self.print_bar([data.0, data.1], chunks, fmt_elapsed);
    }
}

fn find_peaks(_match: &[SampleType], sr: u16, config: Config) -> Vec<find_peaks::Peak<SampleType>> {
    let mut fp = find_peaks::PeakFinder::new(_match);
    fp.with_min_prominence(config.prominence);
    fp.with_min_distance(config.distance.as_secs() as usize * sr as usize);
    fp.find_peaks()
}

fn fft<R: FftNum>(
    planner: &mut RealFftPlanner<R>,
    a: &mut [R],
) -> Result<Box<[Complex<R>]>, realfft::FftError> {
    let len = a.len();
    let r2c = planner.plan_fft_forward(len);

    // make a vector for storing the spectrum
    let mut spectrum = r2c.make_output_vec();

    // Are they the length we expect?
    // assert_eq!(spectrum.len(), len / 2 + 1);
    // assert_eq!(r2c.make_input_vec().len(), len);

    r2c.process(a, &mut spectrum)?;
    Ok(spectrum.into())
}
fn ifft<R: FftNum>(
    planner: &mut RealFftPlanner<R>,
    spectrum: &mut [Complex<R>],
    len: usize,
) -> Result<Box<[R]>, realfft::FftError> {
    let c2r = planner.plan_fft_inverse(len);

    // create a vector for storing the output
    let mut outdata = c2r.make_output_vec();

    // Are they the length we expect?
    // assert_eq!(c2r.make_input_vec().len(), spectrum.len());
    // assert_eq!(outdata.len(), len);

    // inverse transform the spectrum back to a real-valued signal
    c2r.process(spectrum, &mut outdata)?;
    Ok(outdata.into())
}

fn pad<R: FftNum>(a: &[R], len: usize, pad_back: bool) -> Vec<R> {
    let zeros = vec![<R as Zero>::zero(); len - a.len()];
    if pad_back { [a, &zeros] } else { [&zeros, a] }.concat()
}

fn scale_slice<R: FftNum, B: std::ops::Mul<R, Output = B> + Copy>(a: &mut [B], scale: R) {
    for i in 0..a.len() {
        a[i] = a[i] * scale
    }
}
fn pairwise_mult<R: FftNum>(a: &mut [Complex<R>], b: &[Complex<R>]) {
    for i in 0..a.len() {
        a[i] = a[i] * b[i]
    }
}
fn pairwise_mult_w_conj<R: FftNum>(a: &mut [Complex<R>], b: &[Complex<R>]) {
    for i in 0..a.len() {
        a[i] = a[i] * b[i].conj()
    }
}

fn centered<R: FftNum>(out: &[R], len: usize) -> Box<[R]> {
    let start = (out.len() - len) / 2;
    let end = start + len;
    out[start..end].into()
}

pub fn correlate<R: FftNum>(
    a: &[R],
    b: &[R],
    mode: &Mode,
    scale_once: Option<bool>,
    conjugate: bool,
) -> Result<Box<[R]>, realfft::FftError>
where
    R: From<f32>,
{
    let scale: Option<R> = scale_once.map(|scale_once| {
        (1.0 / if scale_once {
            a.len() as f32
        } else {
            (a.len() as f32).sqrt()
        })
        .into()
    });

    let pad_len = a.len() + b.len() - 1;
    let mut a_and_zeros = pad(a, pad_len, !conjugate);
    let mut b_and_zeros = pad(b, pad_len, conjugate);
    if !conjugate {
        b_and_zeros.reverse();
    }

    let mut planner = realfft::RealFftPlanner::<R>::new();
    let mut fft_a = fft(&mut planner, &mut a_and_zeros)?;
    let fft_b = fft(&mut planner, &mut b_and_zeros)?;
    if conjugate {
        pairwise_mult_w_conj(&mut fft_a, &fft_b);
    } else {
        pairwise_mult(&mut fft_a, &fft_b);
    }
    if !scale_once.unwrap_or(true) {
        scale_slice(&mut fft_a, scale.unwrap());
    }
    let mut out = ifft(&mut planner, &mut fft_a, a_and_zeros.len())?;
    // out.rotate_right(pad_len - a.len());

    let mut scalar: R = (1.0 / out.len() as f32).into();
    if let Some(scale) = scale {
        let auto_correlation = *correlate(b, b, &Mode::Valid, None, conjugate)
            .expect("autocorrelation failed")
            .first()
            .expect("autocorrelation empty");

        scalar = scalar * scale / auto_correlation;
    }
    scale_slice(&mut out, scalar);
    Ok(match mode {
        Mode::Full => out.into(),
        Mode::Same => centered(&out, a.len()),
        Mode::Valid => centered(&out, a.len().saturating_sub(b.len()) + 1),
    })
}
pub fn test_data(from: impl Iterator<Item = isize>) -> Vec<f32> {
    from.map(|i| i as f32).collect_vec()
}

#[cfg(test)]
mod correlate_tests {
    use super::*;

    #[test]
    fn correlate_compare_scaling() {
        let mode = Mode::Valid;
        let data1: Vec<f32> = test_data(-10..10);
        let data2: Vec<f32> = vec![1.0, 2.0, 3.0];
        let scaled_once = correlate(&data1, &data2, &mode, Some(true), true)
            .as_ref()
            .unwrap()
            .to_vec();
        let scaled_twice = correlate(&data1, &data2, &mode, Some(false), true)
            .as_ref()
            .unwrap()
            .to_vec();
        assert_float_slice_eq(&scaled_once, &scaled_twice);
        println!("scaled vector is {:?}", &scaled_once)
    }

    #[test]
    fn my_correlate_same_fftcorrelate() {
        let mode = Mode::Valid;
        let data1: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let data2: Vec<f32> = vec![4.0, 5.0];
        let my_conj = correlate(&data1, &data2, &mode, None, true)
            .as_ref()
            .unwrap()
            .to_vec();
        let my = correlate(&data1, &data2, &mode, None, false)
            .as_ref()
            .unwrap()
            .to_vec();
        let expect = fftconvolve::fftcorrelate(
            &Array1::from_iter(data1.into_iter()),
            &Array1::from_iter(data2.into_iter()),
            mode.into(),
        )
        .unwrap()
        .to_vec();
        assert_float_slice_eq(&my, &expect);
        assert_float_slice_eq(&my_conj, &expect);
    }

    fn assert_float_slice_eq(my: &[f32], expect: &[f32]) {
        let mut diff = my.iter().zip(expect).map(|(a, b)| (a - b).abs());
        assert!(
            diff.all(|d| d < 1e-5),
            "expecting {:?} but got {:?}",
            &expect,
            &my
        );
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Full,
    Same,
    Valid,
}

impl From<Mode> for fftconvolve::Mode {
    fn from(value: Mode) -> Self {
        match value {
            Mode::Full => fftconvolve::Mode::Full,
            Mode::Same => fftconvolve::Mode::Same,
            Mode::Valid => fftconvolve::Mode::Valid,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use itertools::Itertools;

    use super::*;

    #[test]
    #[ignore = "slow"]
    fn short_calc_peaks() {
        let snippet_path = PathBuf::from("res/Interlude.mp3");
        let main_path = PathBuf::from("res/small_test.mp3");

        println!("preparing data");
        let sr;
        let s_samples;
        let m_samples;
        {
            let (s_sr, m_sr);
            (s_sr, s_samples) =
                crate::mp3_reader::read_mp3(&snippet_path).expect("invalid snippet mp3");

            (m_sr, m_samples) =
                crate::mp3_reader::read_mp3(&snippet_path).expect("invalid main data mp3");

            if s_sr != m_sr {
                panic!("sample rate dosn't match")
            }
            sr = s_sr;
        }
        println!("prepared data");

        let n = crate::mp3_reader::mp3_duration(&main_path).expect("couln't refind main data file");
        println!("got duration");
        let peaks = calc_chunks(
            sr,
            m_samples,
            s_samples,
            n,
            Config {
                chunk_size: Duration::from_secs(2 * 60),
                overlap_length: crate::mp3_reader::mp3_duration(&snippet_path)
                    .expect("couln't refind snippet data file")
                    / 2,
                distance: Duration::from_secs(5 * 60),
                prominence: 250. as SampleType,
            },
        );
        assert!(peaks
            .into_iter()
            .map(|p| p.position.start / sr as usize)
            .sorted()
            .eq(vec![21, 16 * 60 + 43]));
    }
}
