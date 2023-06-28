use itertools::Itertools;
use std::fs::File;
use std::time::Duration;

mod progress_bar {
    use itertools::Itertools;
    use pad::PadStr;
    use std::io::{stdout, Write};
    pub struct Arrow<'a, const N: usize> {
        pub arrow_prefix: &'a str,
        pub arrow_suffix: &'a str,
        pub arrow_chars: [char; N],
        pub arrow_tip: char,
    }

    impl Default for Arrow<'_, 1> {
        fn default() -> Self {
            Self {
                arrow_prefix: "[",
                arrow_suffix: "]",
                arrow_chars: ['='],
                arrow_tip: '>',
            }
        }
    }
    impl Default for Arrow<'_, 2> {
        fn default() -> Self {
            Self {
                arrow_chars: ['=', '-'],
                arrow_prefix: Arrow::<'_, 1>::default().arrow_prefix,
                arrow_suffix: Arrow::<'_, 1>::default().arrow_suffix,
                arrow_tip: Arrow::<'_, 1>::default().arrow_tip,
            }
        }
    }

    impl<const N: usize> Arrow<'_, N> {
        fn build(&self, fractions: [f64; N], bar_length: usize) -> String {
            let mut arrow = String::new();
            arrow.push_str(self.arrow_prefix);

            for i in 0..N {
                let fraction = fractions[i];
                let char = self.arrow_chars[i];
                arrow.push_str(&char.to_string().repeat(
                    (fraction * bar_length as f64).round() as usize
                        - (arrow.len() - self.arrow_prefix.len()),
                ));
            }

            if bar_length - (arrow.len() - self.arrow_prefix.len()) > 0 {
                arrow.push(self.arrow_tip);
            }
            arrow.push_str(
                " ".repeat(bar_length - (arrow.len() - self.arrow_prefix.len()))
                    .as_str(),
            );
            arrow.push_str(self.arrow_suffix);
            arrow
        }
    }

    pub struct ProgressBar<'a, const N: usize> {
        pub bar_length: usize,
        pub pre_msg: &'a str,
        pub arrow: Arrow<'a, N>,
    }

    impl Default for ProgressBar<'_, 1> {
        fn default() -> Self {
            Self {
                bar_length: 20,
                pre_msg: "Progress: ",
                arrow: Arrow::default(),
            }
        }
    }
    impl Default for ProgressBar<'_, 2> {
        fn default() -> Self {
            Self {
                bar_length: 20,
                pre_msg: "Progress: ",
                arrow: Arrow::default(),
            }
        }
    }

    impl<const N: usize> ProgressBar<'_, N> {
        pub fn print_bar(&self, mut current: [usize; N], total: usize, post_msg: &str) {
            current[N - 1] = current[N - 1].max(0);
            for i in N - 1..0 {
                current[i + 1] = current[i + 1].min(current[i])
            }
            let total = total.max(current[0]);
            let fractions = current.map(|c| c as f64 / total as f64);

            let current_fmt = current
                .iter()
                .map(|f| {
                    f.to_string().pad(
                        (total as f32).log10().ceil() as usize,
                        '0',
                        pad::Alignment::Right,
                        false,
                    )
                })
                .join("+");
            let start = if current[N - 1] == 0 { "" } else { "\r" };
            let ending = if current[0] == total { "\n" } else { "" };

            print!(
                "{start}{}{} {current_fmt}/{}{}{ending}",
                self.pre_msg,
                self.arrow.build(fractions, self.bar_length),
                total,
                post_msg,
            );

            stdout().flush().unwrap();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn empty_arrow() {
            assert_eq!(
                Arrow::default().build([0.0], 10),
                String::from("[>         ]")
            )
        }
        #[test]
        fn short_arrow() {
            assert_eq!(
                Arrow::default().build([0.2], 10),
                String::from("[==>       ]")
            )
        }
        #[test]
        fn long_arrow() {
            assert_eq!(
                Arrow::default().build([0.9], 10),
                String::from("[=========>]")
            )
        }
        #[test]
        fn full_arrow() {
            assert_eq!(
                Arrow::default().build([1.0], 10),
                String::from("[==========]")
            )
        }

        #[test]
        fn double_arrow() {
            assert_eq!(
                Arrow::default().build([0.3, 0.5], 10),
                String::from("[===-->    ]")
            );
        }
    }
}

fn offset_range(range: &std::ops::Range<usize>, offset: usize) -> std::ops::Range<usize> {
    (range.start + offset)..(range.end + offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunked_test() {
        let is = chunked((0..15).into_iter(), 6, 4).collect_vec();
        let expected = vec![0..6, 4..10, 8..14, 12..15]
            .into_iter()
            .map(|r| r.collect_vec())
            .collect_vec();
        assert!(
            &is.eq(&expected),
            "expected {:?} but was {:?}",
            expected,
            is
        );
    }
}

fn chunked<T: Clone>(
    mut data: impl Iterator<Item = T> + 'static,
    window_size: usize,
    hop_length: usize,
) -> impl Iterator<Item = Vec<T>> {
    let mut buffer = Vec::with_capacity(hop_length);
    std::iter::from_fn(move || {
        while buffer.len() < window_size {
            match data.next() {
                Some(e) => buffer.push(e),
                None => break,
            }
        }
        if buffer.is_empty() {
            return None;
        }
        let ret = buffer.clone();
        buffer.drain(..hop_length.min(buffer.len()));

        Some(ret)
    })
}

fn print_offsets(peaks: &Vec<find_peaks::Peak<f64>>, sr: u16) {
    for (i, peak) in peaks
        .iter()
        .sorted_by(|a, b| Ord::cmp(&a.position.start, &b.position.start))
        .enumerate()
    {
        let pos = peak.position.start / sr as usize;
        let seconds = pos % 60;
        let minutes = (pos / 60) % 60;
        let hours = pos / 3600;
        println!(
            "Offset {}: {:0>2}:{:0>2}:{:0>2} with prominence {}",
            i + 1,
            hours,
            minutes,
            seconds,
            &peak.prominence.unwrap()
        );
    }
}

fn open_file_or_exit(snippet_path: &str) -> File {
    match File::open(snippet_path) {
        Ok(file) => file,
        Err(_) => {
            println!("couldn't find file '{snippet_path}'");
            std::process::exit(1);
        }
    }
}

fn main() {
    let snippet_path = "res/Interlude.mp3";
    let main_path = "res/big_test.mp3";

    println!("preparing data");
    let snippet = open_file_or_exit(snippet_path);
    let (s_header, s_samples) = mp3_reader::read_mp3(snippet).expect("invalid snippet mp3");

    let main_data = open_file_or_exit(main_path);
    let (m_header, m_samples) = mp3_reader::read_mp3(main_data).expect("invalid main data mp3");
    println!("prepared data");

    if s_header != m_header {
        panic!("sample rate dosn't match")
    }
    let sr = s_header;

    let n = mp3_reader::mp3_duration(main_path).expect("couln't refind main data file");
    println!("got duration");
    let peaks = audio_matcher::calc_chunks(
        sr,
        m_samples,
        s_samples,
        Duration::from_secs(2 * 60),
        mp3_reader::mp3_duration(snippet_path).expect("couln't refind snippet data file") / 2,
        n,
        Duration::from_secs(5 * 60),
        250.,
    );

    print_offsets(&peaks, sr);

    println!("found peaks {:#?}", &peaks);
}

mod mp3_reader {
    use itertools::Itertools;
    use minimp3::{Decoder, Frame};
    use std::{fs::File, time::Duration};

    // because all samples are 16 bit usage of a single factor is adequat
    const PCM_FACTOR: f64 = 1.0 / (1 << 16 - 1) as f64;
    pub fn read_mp3(file: File) -> Result<(u16, impl Iterator<Item = f64>), minimp3::Error> {
        let (sample_rate, iter) = frame_iterator(Decoder::new(file))?;

        let iter = iter.flat_map(move |frame| {
            if frame.sample_rate as u16 != sample_rate {
                panic!("sample rate changed")
            }
            if frame.channels != 2 {
                panic!("can only handle stereo")
            }
            frame
                .data
                .iter()
                .chunks(2)
                .into_iter()
                .map(|c| {
                    let (l, r) = c.collect_tuple().unwrap();
                    (*l as f64 + *r as f64) * 0.5 * PCM_FACTOR
                })
                .collect::<Vec<_>>()
        });

        Ok((sample_rate, iter))
    }

    struct Wrapper(Decoder<File>);

    impl Iterator for Wrapper {
        type Item = Frame;
        fn next(&mut self) -> Option<Self::Item> {
            match self.0.next_frame() {
                Ok(frame) => Some(frame),
                Err(minimp3::Error::Eof) => None,
                Err(e) => panic!("{:?}", e),
            }
        }
    }

    fn frame_iterator(
        mut decoder: Decoder<File>,
    ) -> Result<(u16, impl Iterator<Item = Frame>), minimp3::Error> {
        let first_frame = decoder.next_frame()?;
        let sample_rate = first_frame.sample_rate as u16;

        Ok((
            sample_rate,
            [first_frame].into_iter().chain(Wrapper(decoder)),
        ))
    }

    pub fn mp3_duration(path: &str) -> std::io::Result<Duration> {
        // first try external bibliothek
        if let Ok(duration) = mp3_duration::from_path(path) {
            return Ok(duration);
        }
        let file = File::open(path)?;

        let decoder = Decoder::new(file);
        let (_, frames) = frame_iterator(decoder).unwrap();
        let seconds: f64 = frames
            // .par_bridge() // parrallel, but seems half as fast
            .map(|frame| {
                frame.data.len() as f64 / (frame.channels as f64 * frame.sample_rate as f64)
            })
            .sum();
        Ok(Duration::from_secs_f64(seconds))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn short_mp3_duration() {
            assert_eq!(mp3_duration("res/Interlude.mp3").unwrap().as_secs(), 7);
        }
        #[test]
        #[ignore = "slow"]
        fn long_mp3_duration() {
            assert_eq!(
                mp3_duration("res/big_test.mp3").unwrap().as_secs(),
                (3 * 60 + 20) * 60 + 55
            );
        }

        #[test]
        fn short_mp3_samples() {
            assert_eq!(
                read_mp3(File::open("res/Interlude.mp3").unwrap())
                    .unwrap()
                    .1
                    .count(),
                323712
            )
        }

        #[test]
        #[ignore = "slow"]
        fn long_mp3_samples() {
            assert_eq!(
                read_mp3(File::open("res/big_test.mp3").unwrap())
                    .unwrap()
                    .1
                    .count(),
                531668736
            )
        }
    }
}

mod audio_matcher {
    use crate::progress_bar::ProgressBar;
    use crate::{chunked, offset_range};
    use fftconvolve::fftcorrelate;
    use find_peaks::{Peak, PeakFinder};
    use itertools::Itertools;
    use ndarray::Array1;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use std::sync::mpsc::channel;
    use threadpool::ThreadPool;

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
        // normalize inputs
        let chunks = ((m_duration).as_secs_f64() / chunk_size.as_secs_f64()).ceil() as usize;
        let overlap_length = (overlap_length.as_secs_f64() * sr as f64).round() as u64;
        let chunk_size = (chunk_size.as_secs_f64() * sr as f64).round() as u64;

        println!("collecting snippet");
        let s_samples: Arc<Array1<f64>> = Arc::new(Array1::from_iter(s_samples));
        let progress_bar = Arc::new(ProgressBar {
            bar_length: chunks.min(80),
            ..ProgressBar::default()
        });
        let progress_state = Arc::new(Mutex::new((0, 0)));

        // threadpool size = Number of Available Cores * (1 + Wait time / Work time)
        // should use less, cause RAM fills up
        let n_workers = 6;
        let pool = ThreadPool::new(n_workers);

        let (tx, rx) = channel::<Vec<Peak<f64>>>();

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
                progress_bar.print_bar([lock.0, lock.1], chunks, "");
                drop(lock);

                let offset = chunk_size as usize * i;
                let m_samples = Array1::from_iter(chunk.into_iter());
                let _matches = fftcorrelate(&m_samples, &s_samples, fftconvolve::Mode::Valid)
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
                progress_bar.print_bar([lock.0, lock.1], chunks, "");
                drop(lock);

                tx.send(peaks)
                    .expect("channel will be there waiting for the pool");
            });
        }

        rx.iter().take(chunks).flatten().collect_vec()
    }

    fn find_peaks(
        _match: &Vec<f64>,
        sr: u16,
        distance: Duration,
        prominence: f64,
    ) -> Vec<find_peaks::Peak<f64>> {
        let mut fp = PeakFinder::new(&_match);
        fp.with_min_prominence(prominence);
        fp.with_min_distance(distance.as_secs() as usize * sr as usize);
        let peaks = fp.find_peaks();
        peaks
    }

    #[cfg(test)]
    mod tests {
        use itertools::Itertools;

        use super::*;

        #[test]
        #[ignore = "slow"]
        fn short_calc_peaks() {
            let snippet_path = "res/Interlude.mp3";
            let main_path = "res/small_test.mp3";

            println!("preparing data");
            let snippet = std::fs::File::open(snippet_path).unwrap();
            let (s_header, s_samples) =
                crate::mp3_reader::read_mp3(snippet).expect("invalid snippet mp3");

            let main_data = std::fs::File::open(main_path).unwrap();
            let (m_header, m_samples) =
                crate::mp3_reader::read_mp3(main_data).expect("invalid main data mp3");
            println!("prepared data");

            if s_header != m_header {
                panic!("sample rate dosn't match")
            }
            let sr = s_header;

            let n =
                crate::mp3_reader::mp3_duration(main_path).expect("couln't refind main data file");
            println!("got duration");
            let peaks = calc_chunks(
                sr,
                m_samples,
                s_samples,
                Duration::from_secs(2 * 60),
                crate::mp3_reader::mp3_duration(snippet_path)
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
}
