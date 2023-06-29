use crate::chunked;
use crate::leveled_output::verbose;
use crate::offset_range;
use crate::progress_bar::ProgressBar;

use find_peaks::Peak;
use itertools::Itertools;

use std::time::{Duration, Instant};

pub fn calc_chunks(
    sr: u16,
    m_samples: impl Iterator<Item = f64> + 'static,
    s_samples: impl Iterator<Item = f64>,
    chunk_size: Duration,
    overlap_length: Duration,
    m_duration: Duration,
    distance: Duration,
    prominence: f64,
) -> Vec<find_peaks::Peak<f64>> {
    use ndarray::Array1;

    use std::sync::{Arc, Mutex};
    use threadpool::ThreadPool;

    // normalize inputs
    let chunks = ((m_duration).as_secs_f64() / chunk_size.as_secs_f64()).ceil() as usize;
    let overlap_length = (overlap_length.as_secs_f64() * sr as f64).round() as u64;
    let chunk_size = (chunk_size.as_secs_f64() * sr as f64).round() as u64;

    verbose(&"collecting snippet");
    let s_samples: Arc<Array1<f64>> = Arc::new(Array1::from_iter(s_samples));
    verbose(&"collected snippet");

    let progress_state = Arc::new(Mutex::new((0, 0)));
    let progress_bar = Arc::new(ProgressBar {
        bar_length: chunks.min(80),
        ..ProgressBar::default()
    }.prepare_output());

    // threadpool size = Number of Available Cores * (1 + Wait time / Work time)
    // should use less, cause RAM fills up
    let n_workers = 6;
    let pool = ThreadPool::new(n_workers);

    let (tx, rx) = std::sync::mpsc::channel::<Vec<Peak<f64>>>();
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
            let peaks = find_peaks(&_matches, sr, distance, prominence)
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

    let ret = rx.iter().take(chunks).flatten().collect_vec();
    Arc::into_inner(progress_bar).expect("reference to Arc<ProgressBar> remaining").finish_output();
    ret
}

impl ProgressBar<'_, 2, crate::progress_bar::Open> {
    fn print_progress(self: &Self, data: &(usize, usize), chunks: usize, start: &Instant) {
        let elapsed = Instant::now().duration_since(*start);
        let (_, minutes, seconds) = crate::split_duration(&elapsed);
        let fmt_elapsed = &format!(" {:0>2}:{:0>2}", minutes, seconds);

        self.print_bar([data.0, data.1], chunks, fmt_elapsed);
    }
}

fn find_peaks(
    _match: &Vec<f64>,
    sr: u16,
    distance: Duration,
    prominence: f64,
) -> Vec<find_peaks::Peak<f64>> {
    let mut fp = find_peaks::PeakFinder::new(&_match);
    fp.with_min_prominence(prominence);
    fp.with_min_distance(distance.as_secs() as usize * sr as usize);
    let peaks = fp.find_peaks();
    peaks
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
            Duration::from_secs(2 * 60),
            crate::mp3_reader::mp3_duration(&snippet_path)
                .expect("couln't refind snippet data file")
                / 2,
            n,
            Duration::from_secs(5 * 60),
            250.,
        );
        assert!(peaks
            .into_iter()
            .map(|p| p.position.start / sr as usize)
            .sorted()
            .eq(vec![21, 16 * 60 + 43]));
    }
}
