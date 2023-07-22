use crate::args::Arguments;
use crate::chunked;
use crate::mp3_reader::SampleType;
use crate::offset_range;
use progress_bar::{Bar, Progress};
use progress_bar::arrow::{Arrow, FancyArrow, SimpleArrow};
use progress_bar::callback::OnceCallback;

use find_peaks::Peak;
use itertools::Itertools;
use ndarray::Array1;
use realfft::{
    num_complex::Complex, num_traits::Zero, ComplexToReal, FftNum, RealFftPlanner, RealToComplex,
};
use std::{
    marker::{Send, Sync},
    sync::{mpsc::Sender, Arc},
    time::Duration,
    vec,
};
use threadpool::ThreadPool;

#[derive(Debug)]
pub struct Config {
    chunk_size: Duration,
    overlap_length: Duration,
    peak_config: PeakConfig,
    threads: usize,
    arrow: Box<dyn Arrow<2> + Send + Sync>,
}
#[derive(Debug, Clone, Copy)]
struct PeakConfig {
    distance: Duration,
    prominence: SampleType,
}
impl Config {
    pub fn from_args(args: &Arguments, s_duration: Duration) -> Self {
        Self {
            chunk_size: Duration::from_secs(args.chunk_size as u64),
            overlap_length: s_duration / 2,
            peak_config: PeakConfig {
                distance: Duration::from_secs(args.distance as u64),
                prominence: args.prominence / 100.0,
            },
            threads: args.threads,
            arrow: if args.fancy_bar {
                Box::<FancyArrow>::default()
            } else {
                Box::<SimpleArrow<2>>::default()
            },
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Full,
    Same,
    Valid,
}

//todo split algo from sample_data
/// represents an Algorythm that can correlate two sets of data.
///
/// It should know the data of the sample, and its autocorrelation to optimize multiple calls with the same sample
pub trait CorrelateAlgo<R: FftNum + From<f32>> {
    fn inverse_sample_auto_correlation(&self) -> R;
    fn correlate_with_sample(
        &self,
        within: &[R],
        mode: Mode,
        scale: bool,
    ) -> Result<Vec<R>, Box<dyn std::error::Error>>;
    fn scale(&self, data: &mut [R]) {
        scale_slice(data, self.inverse_sample_auto_correlation());
    }
}

impl From<Mode> for fftconvolve::Mode {
    fn from(value: Mode) -> Self {
        match value {
            Mode::Full => Self::Full,
            Mode::Same => Self::Same,
            Mode::Valid => Self::Valid,
        }
    }
}

pub fn calc_chunks<C: CorrelateAlgo<SampleType> + Sync + Send + 'static>(
    sr: u16,
    m_samples: impl Iterator<Item = SampleType> + Send + Sync + 'static,
    algo_with_sample: C,
    m_duration: Duration,
    scale: bool,
    config: Config,
) -> Vec<find_peaks::Peak<SampleType>> {
    // normalize inputs
    let chunks = (m_duration.as_secs_f64() / config.chunk_size.as_secs_f64()).ceil() as usize;
    let overlap_length = (config.overlap_length.as_secs_f64() * sr as f64).round() as u64;
    let chunk_size = (config.chunk_size.as_secs_f64() * sr as f64).round() as u64;

    let algo_with_sample = Arc::new(algo_with_sample);

    // threadpool size = Number of Available Cores * (1 + Wait time / Work time)
    // should use less, cause RAM fills up, 3 seems to be enough to saturate decoder
    let n_workers = config.threads;
    let pool = ThreadPool::new(n_workers);
    let (tx, rx) = std::sync::mpsc::channel::<Vec<Peak<SampleType>>>();

    let mut progress = Progress::new_external_bound(
        chunked(
            m_samples,
            chunk_size as usize + overlap_length as usize,
            chunk_size as usize,
        )
        .enumerate(),
        Bar::new("Progress: ".to_string(), true, config.arrow), // TODO maybe move Bar to config
        0,
        chunks,
    );
    if let Some(width) = progress_bar::terminal_width() {
        progress.set_max_len(width);
    }
    let (iter, holder) = progress.into();

    for (i, chunk) in iter {
        assert!(chunks > i, "to many chunks");

        let [f1, f2] = OnceCallback::new(&holder);

        let algo_with_sample = Arc::clone(&algo_with_sample);
        let tx = Sender::clone(&tx);
        pool.execute(move || {
            f1.call();

            let offset = chunk_size as usize * i;
            let matches = algo_with_sample
                .correlate_with_sample(&chunk, Mode::Valid, scale)
                .unwrap();

            let peaks = find_peaks(&matches, sr, config.peak_config)
                .into_iter()
                .update(|p| p.position = offset_range(&p.position, offset))
                .collect::<Vec<_>>();

            tx.send(peaks).unwrap();
            f2.call();
        });
    }

    pool.join();

    assert!(pool.panic_count() == 0, "some worker threads paniced");
    rx.into_iter()
        .take(chunks)
        .flatten()
        .sorted_by(|a, b| Ord::cmp(&a.position.start, &b.position.start))
        .collect_vec()
}

fn find_peaks(
    y_data: &[SampleType],
    sr: u16,
    config: PeakConfig,
) -> Vec<find_peaks::Peak<SampleType>> {
    let mut fp = find_peaks::PeakFinder::new(y_data);
    fp.with_min_prominence(config.prominence);
    fp.with_min_distance(config.distance.as_secs() as usize * sr as usize);
    fp.find_peaks()
}

fn pad<R: Zero + Clone>(a: &[R], len: usize, pad_back: bool) -> Vec<R> {
    let zeros = vec![R::zero(); len - a.len()];
    if pad_back { [a, &zeros] } else { [&zeros, a] }.concat()
}

fn map_in_place<T, F>(a: &mut [T], map: F)
where
    T: Copy,
    F: Fn(T) -> T,
{
    for element in a {
        *element = map(*element);
    }
}
fn scale_slice<S, T>(a: &mut [T], scale: S)
where
    S: Copy,
    T: std::ops::Mul<S, Output = T> + Copy,
{
    map_in_place(a, |f| f * scale);
}

fn pairwise_map_in_place<T1, T2, F>(a: &mut [T1], b: &[T2], map: F)
where
    T1: Copy,
    T2: Copy,
    F: Fn(T1, T2) -> T1,
{
    assert_eq!(a.len(), b.len(), "can only map elements of same lenght");
    for (i, element) in a.iter_mut().enumerate() {
        *element = map(*element, b[i]);
    }
}

fn pairwise_mult_in_place<R, F>(a: &mut [R], b: &[R], map: F)
where
    R: std::ops::Mul<Output = R> + Copy,
    F: Fn(R) -> R,
{
    pairwise_map_in_place(a, b, |x, y| x * map(y));
}

#[allow(dead_code)]
fn pairwise_add_in_place<R>(a: &mut [R], b: &[R])
where
    R: std::ops::Add<Output = R> + Copy,
{
    pairwise_map_in_place(a, b, |x, y| x + y);
}

pub struct LibConvolve {
    sample_data: Box<[SampleType]>,
    inv_sample_auto_corrolation: lazy_init::Lazy<SampleType>,
    sample_array: lazy_init::Lazy<Array1<SampleType>>,
}
impl LibConvolve {
    pub fn new(sample_data: Box<[SampleType]>) -> Self {
        Self {
            sample_data,
            inv_sample_auto_corrolation: lazy_init::Lazy::new(),
            sample_array: lazy_init::Lazy::new(),
        }
    }

    fn correlate(
        &self,
        within: &Array1<SampleType>,
        sample: &Array1<SampleType>,
        mode: Mode,
        scale: bool,
    ) -> Result<Vec<SampleType>, Box<dyn std::error::Error>> {
        let mode: fftconvolve::Mode = <Mode as Into<fftconvolve::Mode>>::into(mode);
        let mut res = fftconvolve::fftcorrelate(within, sample, mode)?.to_vec();
        if scale {
            self.scale(&mut res);
        }
        Ok(res)
    }
    fn convert_data(raw: &[SampleType]) -> Array1<SampleType> {
        Array1::from_iter(raw.iter().copied())
    }

    fn sample_array(&self) -> &Array1<SampleType> {
        self.sample_array
            .get_or_create(|| Self::convert_data(&self.sample_data))
    }
}
impl CorrelateAlgo<SampleType> for LibConvolve {
    fn inverse_sample_auto_correlation(&self) -> SampleType {
        *self.inv_sample_auto_corrolation.get_or_create(|| {
            1.0 / self
                .correlate(self.sample_array(), self.sample_array(), Mode::Valid, false)
                .expect("autocorrelation failed")
                .first()
                .expect("auto correlation empty")
        })
    }

    fn correlate_with_sample(
        &self,
        within: &[SampleType],
        mode: Mode,
        scale: bool,
    ) -> Result<Vec<SampleType>, Box<dyn std::error::Error>> {
        self.correlate(
            &Self::convert_data(within),
            self.sample_array(),
            mode,
            scale,
        )
    }
}

struct MyR2C2C<R: FftNum>(Arc<dyn RealToComplex<R>>, Arc<dyn ComplexToReal<R>>);
impl<R: FftNum> MyR2C2C<R> {
    fn new(planner: &mut RealFftPlanner<R>, len: usize) -> Self {
        Self(
            Arc::clone(&planner.plan_fft_forward(len)),
            Arc::clone(&planner.plan_fft_inverse(len)),
        )
    }
    fn fft(&self, a: &mut [R]) -> Result<Vec<Complex<R>>, realfft::FftError> {
        // make a vector for storing the spectrum
        let mut spectrum = self.0.make_output_vec();

        // Are they the length we expect?
        // assert_eq!(spectrum.len(), len / 2 + 1);
        // assert_eq!(r2c.make_input_vec().len(), len);

        self.0.process(a, &mut spectrum)?;
        Ok(spectrum)
    }
    fn ifft(&self, spectrum: &mut [Complex<R>]) -> Result<Vec<R>, realfft::FftError> {
        // create a vector for storing the output
        let mut outdata = self.1.make_output_vec();

        // Are they the length we expect?
        // assert_eq!(c2r.make_input_vec().len(), spectrum.len());
        // assert_eq!(outdata.len(), len);

        // inverse transform the spectrum back to a real-valued signal
        self.1.process(spectrum, &mut outdata)?;
        Ok(outdata)
    }
}

pub struct MyConvolve<R: FftNum> {
    planner: std::sync::Mutex<RealFftPlanner<R>>,
    sample_data: Box<[R]>,
    inv_sample_auto_corrolation: lazy_init::Lazy<R>,
    pub use_conjugation: bool,
}
impl<R: FftNum + From<f32>> MyConvolve<R> {
    pub fn new_with_planner(planner: RealFftPlanner<R>, sample_data: Box<[R]>) -> Self {
        Self {
            planner: std::sync::Mutex::new(planner),
            sample_data,
            inv_sample_auto_corrolation: lazy_init::Lazy::new(),
            use_conjugation: true,
        }
    }
    pub fn new(sample_data: Box<[R]>) -> Self {
        Self {
            planner: std::sync::Mutex::new(RealFftPlanner::<R>::new()),
            sample_data,
            inv_sample_auto_corrolation: lazy_init::Lazy::new(),
            use_conjugation: true,
        }
    }
    fn _inverse_sample_auto_correlation(&self) -> R {
        *self.inv_sample_auto_corrolation.get_or_create(|| {
            R::from(1.0)
                / *self
                    .correlate_with_sample(&self.sample_data, Mode::Valid, false)
                    .expect("autocorrelation failed")
                    .first()
                    .expect("autocorrelation yeildet wrong no output")
        })
    }
    pub fn correlate(
        &self,
        within: &[R],
        sample: &[R],
        mode: Mode,
        scale: bool,
    ) -> Result<Vec<R>, realfft::FftError> {
        let pad_len = within.len() + sample.len() - 1;
        let mut within_and_zeros = pad(within, pad_len, !self.use_conjugation);
        let mut sample_and_zeros = pad(sample, pad_len, self.use_conjugation);
        if !self.use_conjugation {
            sample_and_zeros.reverse();
        }
        let r2c2r = MyR2C2C::new(&mut self.planner.lock().unwrap(), pad_len);

        let mut fft_a = r2c2r.fft(&mut within_and_zeros)?;
        let fft_b = r2c2r.fft(&mut sample_and_zeros)?;

        pairwise_mult_in_place(&mut fft_a, &fft_b, |b| {
            if self.use_conjugation {
                b.conj()
            } else {
                b
            }
        });

        let mut out = r2c2r.ifft(&mut fft_a)?;

        let mut scalar: R = (1.0 / out.len() as f32).into(); // needed scaling
        if scale {
            let scale: R = (within.len() as f32).into(); // removes fft induced factor
            let auto_correlation = self._inverse_sample_auto_correlation(); // scales from [-1,1]

            scalar = scalar * auto_correlation / scale;
        }
        scale_slice(&mut out, scalar);
        Ok(match mode {
            Mode::Full => out,
            Mode::Same => Self::centered(&out, within.len()).into(),
            Mode::Valid => {
                Self::centered(&out, within.len().saturating_sub(sample.len()) + 1).into()
            }
        })
    }

    /// returns a slice with a length `len` centered in the middle of `out`
    fn centered(arr: &[R], len: usize) -> &[R] {
        let start = (arr.len() - len) / 2;
        let end = start + len;
        arr[start..end].into()
    }
}
impl<R: FftNum + From<f32>> CorrelateAlgo<R> for MyConvolve<R> {
    fn inverse_sample_auto_correlation(&self) -> R {
        self._inverse_sample_auto_correlation()
    }

    fn correlate_with_sample(
        &self,
        within: &[R],
        mode: Mode,
        scale: bool,
    ) -> Result<Vec<R>, Box<dyn std::error::Error>> {
        Ok(self.correlate(within, &self.sample_data, mode, scale)?)
    }
}

pub fn test_data(from: impl Iterator<Item = isize>) -> Vec<f32> {
    from.map(|i| i as f32).collect_vec()
}

#[cfg(test)]
mod correlate_tests {
    use super::*;

    #[test]
    fn my_correlate_same_fftcorrelate() {
        let scale = false;
        let mode = Mode::Valid;
        let data1: Vec<f32> = test_data(-10..10);
        let data2: Vec<f32> = vec![1.0, 2.0, 3.0];

        let mut my_algo = MyConvolve::new(data2.clone().into());
        let lib_algo = LibConvolve::new(data2.into());

        let my_conj = my_algo.correlate_with_sample(&data1, mode, scale).unwrap();

        my_algo.use_conjugation = false;
        let my = my_algo.correlate_with_sample(&data1, mode, scale).unwrap();
        let expect = lib_algo.correlate_with_sample(&data1, mode, scale).unwrap();
        assert_float_slice_eq(&my, &expect);
        assert_float_slice_eq(&my_conj, &expect);
    }

    fn assert_float_slice_eq(my: &[f32], expect: &[f32]) {
        let mut diff = my.iter().zip(expect).map(|(a, b)| (a - b).abs());
        assert!(
            diff.all(|d| d < 1.2e-5),
            "expecting \n{:?} but got \n{:?} with diff \n{:?}",
            &expect,
            &my,
            &diff.collect_vec()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;
    use std::path::PathBuf;

    #[test]
    #[ignore = "slow"]
    fn short_calc_peaks() {
        let snippet_path = PathBuf::from("res/local/Interlude.mp3");
        let main_path = PathBuf::from("res/local/small_test.mp3");

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
        let algo = LibConvolve::new(s_samples.collect::<Box<[_]>>());
        println!("prepared data");

        let n = crate::mp3_reader::mp3_duration(&main_path, false)
            .expect("couln't refind main data file");
        println!("got duration");
        let peaks = calc_chunks(
            sr,
            m_samples,
            algo,
            n,
            false,
            Config {
                chunk_size: Duration::from_secs(60),
                overlap_length: crate::mp3_reader::mp3_duration(&snippet_path, false)
                    .expect("couln't refind snippet data file")
                    / 2,
                peak_config: PeakConfig {
                    distance: Duration::from_secs(8 * 60),
                    prominence: 15. as SampleType,
                },
                threads: 6,
                arrow: Box::new(SimpleArrow::default()),
            },
        );
        assert!(peaks
            .into_iter()
            .map(|p| p.position.start / sr as usize)
            .sorted()
            .eq(vec![21, 16 * 60 + 43]));
    }
}
