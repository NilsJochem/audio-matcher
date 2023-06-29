mod progress_bar;
mod mp3_reader;
mod audio_matcher;
mod errors;
pub mod args;
pub mod leveled_output;

use std::time::Duration;

use leveled_output::{info, debug, verbose};
use itertools::Itertools;

fn offset_range(range: &std::ops::Range<usize>, offset: usize) -> std::ops::Range<usize> {
    (range.start + offset)..(range.end + offset)
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
        info(&format!(
            "Offset {}: {:0>2}:{:0>2}:{:0>2} with prominence {}",
            i + 1,
            hours,
            minutes,
            seconds,
            &peak.prominence.unwrap()
        ));
    }
}


pub fn run(args: args::Arguments) -> Result<(), Box<dyn std::error::Error>> {
    unsafe { crate::leveled_output::OUTPUT_LEVEL = args.output_level.clone().into() };
    debug(&format!("{:#?}", args));

    let snippet_path = args.snippet;
    let main_path = args.within.first().unwrap();

    verbose(&"preparing data");
    let sr;
    let s_samples;
    let m_samples;
    {
        let (s_sr, m_sr);
        (s_sr, s_samples) = mp3_reader::read_mp3(&snippet_path)?;
        (m_sr, m_samples) = mp3_reader::read_mp3(&main_path)?;

        if s_sr != m_sr {
            return Err(Box::new(errors::SampleRateMismatch(Box::new([s_sr, m_sr]))));
        }
        sr = s_sr;
    }
    verbose(&"prepared data");

    let m_duration = mp3_reader::mp3_duration(&main_path)?;
    let s_duration = mp3_reader::mp3_duration(&snippet_path)?;
    verbose(&"got duration");
    let peaks = audio_matcher::calc_chunks(
        sr,
        m_samples,
        s_samples,
        Duration::from_secs(args.chunk_size as u64),
        s_duration / 2,
        m_duration,
        Duration::from_secs(args.distance as u64),
        args.prominence,
    );

    print_offsets(&peaks, sr);

    debug(&format!("found peaks {:#?}", &peaks));
    Ok(())
}

