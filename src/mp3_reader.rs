use itertools::Itertools;
use minimp3::{Decoder, Frame};
use std::{error::Error, fs::File, time::Duration};

use crate::errors::{NoFile, NoMp3};
use crate::leveled_output::verbose;

// because all samples are 16 bit usage of a single factor is adequat
const PCM_FACTOR: f64 = 1.0 / (1 << 16 - 1) as f64;
pub fn read_mp3<'a, P>(
    path: &'a P,
) -> Result<(u16, impl Iterator<Item = f64> + 'static), Box<dyn Error + 'static>>
where
    P: AsRef<std::path::Path>,
{
    let file = File::open(&path).map_err(|_| NoFile::new(path))?;
    let (sample_rate, iter) =
        frame_iterator(Decoder::new(file)).map_err(|_| NoMp3::new(path))?;

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

pub fn mp3_duration<P>(path: &P) -> Result<Duration, Box<dyn Error>>
where
    P: AsRef<std::path::Path>,
{
    // first try external bibliothek
    if let Ok(duration) = mp3_duration::from_path(path) {
        return Ok(duration);
    }
    verbose(&"fallback to own implementation for mp3_duration");
    let file = File::open(path).map_err(|_| NoFile::new(path))?;

    let decoder = Decoder::new(file);
    let (_, frames) = frame_iterator(decoder).map_err(|_| NoMp3::new(path))?;
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
        assert_eq!(mp3_duration(&"res/Interlude.mp3").unwrap().as_secs(), 7);
    }
    #[test]
    #[ignore = "slow"]
    fn long_mp3_duration() {
        assert_eq!(
            mp3_duration(&"res/big_test.mp3").unwrap().as_secs(),
            (3 * 60 + 20) * 60 + 55
        );
    }

    #[test]
    fn short_mp3_samples() {
        assert_eq!(read_mp3(&"res/Interlude.mp3").unwrap().1.count(), 323712)
    }

    #[test]
    #[ignore = "slow"]
    fn long_mp3_samples() {
        assert_eq!(read_mp3(&"res/big_test.mp3").unwrap().1.count(), 531668736)
    }
}
