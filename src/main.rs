use itertools::Itertools;
use std::fs::File;
use std::time::Duration;

mod progress_bar {
    use pad::PadStr;
    use std::io::{stdout, Write};
    pub(crate) struct Arrow<'a> {
        pub(crate) arrow_prefix: &'a str,
        pub(crate) arrow_suffix: &'a str,
        pub(crate) arrow_char: char,
        pub(crate) arrow_tip: char,
    }

    impl Default for Arrow<'_> {
        fn default() -> Self {
            Self {
                arrow_prefix: "[",
                arrow_suffix: "]",
                arrow_char: '=',
                arrow_tip: '>',
            }
        }
    }

    impl Arrow<'_> {
        fn build(&self, fraction: f64, bar_length: usize) -> String {
            let mut arrow = String::new();
            arrow.push_str(self.arrow_prefix);

            arrow.push_str(
                &self
                    .arrow_char
                    .to_string()
                    .repeat(((fraction * bar_length as f64) as usize).saturating_sub(1)),
            );
            arrow.push(self.arrow_tip);
            arrow.push_str(
                " ".repeat(self.arrow_prefix.len() + bar_length - arrow.len())
                    .as_str(),
            );
            arrow.push_str(self.arrow_suffix);
            arrow
        }
    }

    pub(crate) struct ProgressBar<'a> {
        pub(crate) total: usize,
        pub(crate) bar_length: usize,
        pub(crate) pre_msg: &'a str,
        pub(crate) arrow: Arrow<'a>,
    }

    impl ProgressBar<'_> {
        pub(crate) fn new<'a>(
            total: usize,
            bar_length: Option<usize>,
            pre_msg: Option<&'a str>,
            arrow: Arrow<'a>,
        ) -> ProgressBar<'a> {
            ProgressBar {
                total,
                bar_length: bar_length.unwrap_or(20),
                pre_msg: pre_msg.unwrap_or("'Progress: '"),
                arrow,
            }
        }

        pub(crate) fn print_bar(&self, current: usize, post_msg: &str) {
            let total = self.total.max(current);
            let fraction = current as f64 / total as f64;

            let current_fmt = current.to_string().pad(
                (current as f32).log10().ceil() as usize,
                '0',
                pad::Alignment::Right,
                false,
            );
            let start = if current == 0 { "" } else { "\r" };
            let ending = if current == total { "\n" } else { "" };

            print!(
                "{start}{}{} {current_fmt}/{}{}{ending}",
                self.pre_msg,
                self.arrow.build(fraction, self.bar_length),
                total,
                post_msg,
            );

            stdout().flush().unwrap();
        }
    }
}

fn offset_range(range: &std::ops::Range<usize>, offset: usize) -> std::ops::Range<usize> {
    (range.start + offset)..(range.end + offset)
}

#[test]
fn chunked_test() {
    let is = chunked((1..5).into_iter(), 3, 2).collect_vec();
    let expected = vec![vec![1, 2, 3], vec![3, 4]];
    assert!(
        &is.eq(&expected),
        "expected {:?} but was {:?}",
        expected,
        is
    );

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

fn chunked<T: Clone>(
    mut data: impl Iterator<Item = T> + 'static,
    window_size: usize,
    hop_length: usize,
) -> impl Iterator<Item = Vec<T>> {
    let mut buffer = Vec::new();
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

fn main() {
    let snippet_path = "res/Interlude.mp3";
    let main_path = "res/big_test.mp3";

    println!("preparing data");
    let snippet = File::open(snippet_path).expect("couldn't find snippet file");
    let (s_header, s_samples) = mp3_reader::read_mp3(snippet).expect("invalid snippet mp3");

    let main_data = File::open(main_path).expect("couln't find main data file");
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
    use minimp3::Decoder;
    use std::{fs::File, time::Duration};

    // because all samples are 16 bit usage of a single factor is adequat
    const PCM_FACTOR: f64 = 1.0 / (1 << 16 - 1) as f64;
    pub(crate) fn read_mp3(file: File) -> Result<(u16, impl Iterator<Item = f64>), minimp3::Error> {
        let mut decoder = Decoder::new(file);
        let mut frame = decoder.next_frame()?;

        let sr = frame.sample_rate as u16;
        let mut i = 0;

        let iter = std::iter::from_fn(move || {
            if i >= frame.data.len() {
                frame = match decoder.next_frame() {
                    Ok(frame) => {
                        if frame.sample_rate as u16 != sr {
                            panic!("sample rate changed")
                        }
                        frame
                    }
                    Err(minimp3::Error::Eof) => return None,
                    Err(e) => panic!("{:?}", e),
                };
                i = 0;
            }
            let sample = match frame.channels {
                1 => frame.data[i] as f64,
                2 => (frame.data[i] as i32 + frame.data[i + 1] as i32) as f64 * 0.5,
                x => panic!("can't handle {x} channels"),
            } * PCM_FACTOR;
            i += frame.channels;
            Some(sample)
        });
        Ok((sr, iter))
    }

    pub(crate) fn mp3_duration(path: &str) -> std::io::Result<Duration> {
        // first try external bibliothek
        if let Ok(duration) = mp3_duration::from_path(path) {
            return Ok(duration);
        }
        let file = File::open(path)?;
        let mut decoder = Decoder::new(file);
        let mut seconds = 0.0;
        while let Ok(frame) = decoder.next_frame() {
            seconds += frame.data.len() as f64 / (frame.channels as f64 * frame.sample_rate as f64)
        }
        Ok(Duration::from_secs_f64(seconds))
    }
}

mod audio_matcher {
    use crate::{offset_range, chunked};
    use crate::progress_bar::{ProgressBar, Arrow};
    use std::time::Duration;
    use fftconvolve::fftcorrelate;
    use find_peaks::PeakFinder;
    use ndarray::Array1;

    pub(crate) fn calc_chunks(
        sr: u16,
        m_samples: impl Iterator<Item = f64> + 'static,
        s_samples: impl Iterator<Item = f64>,
        chunk_size: Duration,
        overlap_length: Duration,
        duration: Duration,
        distance: Duration,
        prominence: f64,
    ) -> Vec<find_peaks::Peak<f64>> {
        // normalize inputs
        let chunk_size = chunk_size.as_secs() * sr as u64;
        let overlap_length = overlap_length.as_secs() * sr as u64;
        let chunks = (duration.as_secs() as f64 * sr as f64 / chunk_size as f64).ceil() as usize;

        let s_samples: Array1<f64> = Array1::from_iter(s_samples);
        let progress_bar = ProgressBar::new(
            chunks,
            Some(chunks.min(80)),
            None,
            Arrow {
                arrow_char: '-',
                arrow_tip: '-',
                ..Default::default()
            },
        );

        let mut peaks = Vec::new();
        for (i, chunk) in chunked(
            m_samples,
            chunk_size as usize + overlap_length as usize,
            chunk_size as usize,
        )
        .enumerate()
        {
            progress_bar.print_bar(i + 1, "");
            let offset = chunk_size as usize * i;
            let m_samples = Array1::from_iter(chunk.into_iter());
            let _matches = fftcorrelate(&m_samples, &s_samples, fftconvolve::Mode::Valid)
                .unwrap()
                .to_vec();
            peaks.extend(
                find_peaks(&_matches, sr, distance, prominence)
                    .iter()
                    .map(|p| {
                        let mut p = p.clone();
                        p.position = offset_range(&p.position, offset);
                        p
                    }),
            );
        }
        peaks
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
}
