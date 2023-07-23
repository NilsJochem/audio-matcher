use itertools::Itertools;
use minimp3::{Decoder, Frame};
use rayon::prelude::*;
use std::{fs::File, time::Duration};

use crate::matcher::{errors::CliError::{self, NoFile, NoMp3}, verbose};

pub type SampleType = f32;

// because all samples are 16 bit usage of a single factor is adequat
const PCM_FACTOR: SampleType = 1.0 / ((1 << 16) - 1) as SampleType;
pub fn read_mp3<P>(path: &P) -> Result<(u16, impl Iterator<Item = SampleType> + 'static), CliError>
where
    P: AsRef<std::path::Path>,
{
    let file = File::open(path).map_err(|_| NoFile(path.into()))?;
    let (sample_rate, iter) = frame_iterator(Decoder::new(file)).map_err(|_| NoMp3(path.into()))?;

    let iter = iter.flat_map(move |frame| {
        assert!(
            frame.sample_rate as u16 == sample_rate,
            "sample rate changed from {sample_rate} to {}",
            frame.sample_rate
        );
        assert!(frame.channels == 2, "can only handle stereo");

        frame
            .data
            .iter()
            .chunks(2)
            .into_iter()
            .map(|c| {
                let (l, r) = c.collect_tuple().unwrap();
                (*l as SampleType + *r as SampleType) * 0.5 * PCM_FACTOR
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
            Err(e) => panic!("{e:?}"),
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
        std::iter::once(first_frame).chain(Wrapper(decoder)),
    ))
}

pub fn mp3_duration<P>(path: &P, use_parallel: bool) -> Result<Duration, CliError>
where
    P: AsRef<std::path::Path>,
{
    // first try external bibliothek
    if let Ok(duration) = mp3_duration::from_path(path) {
        return Ok(duration);
    }
    verbose!("fallback to own implementation for mp3_duration");

    let file = File::open(path).map_err(|_| NoFile(path.into()))?;

    let decoder = Decoder::new(file);
    let (_, frames) = frame_iterator(decoder).map_err(|_| NoMp3(path.into()))?;
    let seconds: f64 = if use_parallel {
        frames
            .par_bridge() // parrallel, but seems half as fast
            .map(|frame| {
                frame.data.len() as f64 / (frame.channels as f64 * frame.sample_rate as f64)
            })
            .sum()
    } else {
        frames
            .map(|frame| {
                frame.data.len() as f64 / (frame.channels as f64 * frame.sample_rate as f64)
            })
            .sum()
    };
    Ok(Duration::from_secs_f64(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_mp3_duration() {
        assert_eq!(
            mp3_duration(&"res/Interlude.mp3", false).unwrap().as_secs(),
            7
        );
    }
    #[test]
    #[ignore = "slow"]
    fn long_mp3_duration() {
        assert_eq!(
            mp3_duration(&"res/big_test.mp3", false).unwrap().as_secs(),
            (3 * 60 + 20) * 60 + 55
        );
    }

    #[test]
    fn short_mp3_samples() {
        assert_eq!(read_mp3(&"res/Interlude.mp3").unwrap().1.count(), 323_712);
    }

    #[test]
    #[ignore = "slow"]
    fn long_mp3_samples() {
        assert_eq!(
            read_mp3(&"res/big_test.mp3").unwrap().1.count(),
            531_668_736
        );
    }
}
