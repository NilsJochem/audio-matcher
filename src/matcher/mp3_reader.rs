use itertools::Itertools;
use log::trace;
use minimp3::{Decoder, Frame};
use rayon::prelude::*;
use std::{fs::File, path::Path, time::Duration};

use crate::matcher::errors::CliError::{self, NoFile, NoMp3};

pub type SampleType = f32;

// because all samples are 16 bit usage of a single factor is adequat
const PCM_FACTOR: SampleType = 1.0 / ((1 << 16) - 1) as SampleType;
pub fn read_mp3(
    path: impl AsRef<Path>,
) -> Result<(u16, impl Iterator<Item = SampleType> + 'static), CliError> {
    let path = path.as_ref();
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

pub fn mp3_duration(path: impl AsRef<Path>, use_parallel: bool) -> Result<Duration, CliError> {
    use crate::worker::tagger;
    let path = path.as_ref();
    let tag = tagger::TaggedFile::from_path(path.to_path_buf(), false).ok();
    // first try reading from tags or with external bibliothek
    if let Some(duration) = tag
        .as_ref()
        .and_then(tagger::TaggedFile::get::<tagger::Length>)
        .or_else(|| mp3_duration::from_path(path).ok())
    {
        return Ok(duration);
    }
    drop(tag);
    trace!("fallback to own implementation for mp3_duration");

    let file = File::open(path).map_err(|_| NoFile(path.into()))?;

    let decoder = Decoder::new(file);
    let (_, frames) = frame_iterator(decoder).map_err(|_| NoMp3(path.into()))?;
    let duration = Duration::from_secs_f64(if use_parallel {
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
    });
    // save duration in tags, read new, in case somthing changed
    let mut tag = tagger::TaggedFile::from_path(path.to_path_buf(), true)
        .map_err(|err| CliError::ID3(path.into(), err))?;
    tag.set::<tagger::Length>(duration);
    tag.save_changes(false)
        .map_err(|err| CliError::ID3(path.into(), err))?;
    Ok(duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_mp3_duration() {
        assert_eq!(
            mp3_duration("res/local/Interlude.mp3", false)
                .unwrap()
                .as_secs(),
            7
        );
    }
    #[test]
    #[ignore = "slow"]
    fn long_mp3_duration() {
        assert_eq!(
            mp3_duration("res/local/big_test.mp3", false)
                .unwrap()
                .as_secs(),
            (3 * 60 + 20) * 60 + 55
        );
    }

    #[test]
    fn short_mp3_samples() {
        assert_eq!(
            read_mp3("res/local/Interlude.mp3").unwrap().1.count(),
            323_712
        );
    }

    #[test]
    #[ignore = "slow"]
    fn long_mp3_samples() {
        assert_eq!(
            read_mp3("res/local/big_test.mp3").unwrap().1.count(),
            531_668_736
        );
    }
}
