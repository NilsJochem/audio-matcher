pub mod args;
pub mod audio_matcher;
mod errors;
pub mod leveled_output;
pub mod mp3_reader;
pub mod progress_bar;

use std::{time::Duration, usize};

use errors::CliError;
use find_peaks::Peak;
use itertools::Itertools;
use leveled_output::{debug, error, info, verbose};
use mp3_reader::SampleType;
use text_io::read;

const fn offset_range(range: &std::ops::Range<usize>, offset: usize) -> std::ops::Range<usize> {
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

fn print_offsets(peaks: &[find_peaks::Peak<SampleType>], sr: u16) {
    if peaks.is_empty() {
        info(&"no offsets found");
    }
    for (i, peak) in peaks
        .iter()
        .sorted_by(|a, b| Ord::cmp(&a.position.start, &b.position.start))
        .enumerate()
    {
        let pos = peak.position.start / sr as usize;
        let (hours, minutes, seconds) = crate::split_duration(&Duration::from_secs(pos as u64));
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

#[inline]
pub const fn split_duration(duration: &Duration) -> (usize, usize, usize) {
    let elapsed = duration.as_secs() as usize;
    let seconds = elapsed % 60;
    let minutes = (elapsed / 60) % 60;
    let hours = elapsed / 3600;
    (hours, minutes, seconds)
}

pub fn run(args: args::Arguments) -> Result<(), CliError> {
    unsafe { crate::leveled_output::OUTPUT_LEVEL = args.output_level.into(); }
    debug(&format!("{args:#?}"));

    let snippet_path = &args.snippet;
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
            return Err(errors::CliError::SampleRateMismatch(s_sr, m_sr));
        }
        sr = s_sr;
    }
    verbose(&"prepared data");
    let sample_data = s_samples.collect::<Box<[_]>>();
    verbose(&"collected snippet");
    let algo = audio_matcher::LibConvolve::new(sample_data);

    let s_duration = mp3_reader::mp3_duration(&snippet_path, false)?;
    let m_duration = mp3_reader::mp3_duration(&main_path, false)?;
    verbose(&"got duration");
    let peaks = audio_matcher::calc_chunks(
        sr,
        m_samples,
        algo,
        m_duration,
        true,
        audio_matcher::Config::from_args(&args, s_duration),
    );

    print_offsets(&peaks, sr);
    debug(&format!("found peaks {:#?}", &peaks));

    info(&"");
    if let Some(out_path) = args
        .out_file
        .out_file
        .or_else(|| {
            (!args.out_file.no_out).then(|| {
                let mut path = main_path.clone();
                path.set_extension("txt");
                path
            })
        })
        .filter(|path| {
            let out = !std::path::Path::new(path).exists()
                || ask_consent(
                    &format!("file '{}' already exists, overwrite", path.display()),
                    args.always_answer,
                );
            if !out {
                error(&format!("won't overwrite '{}'", path.display()));
            }
            out
        })
    {
        verbose(&format!("writing result to '{}'", out_path.display()));
        write_text_marks(
            &peaks,
            sr as SampleType,
            &out_path,
            Duration::from_secs(7),
            args.dry_run,
        )?;
    }

    Ok(())
}

fn ask_consent(msg: &str, args: args::Inputs) -> bool {
    if args.yes || args.no {
        return args.yes;
    }
    print!("{msg} [y/n]: ");
    for _ in std::iter::repeat(args.trys - 1) {
        let rin: String = read!("{}\n");
        if ["y", "yes", "j", "ja"].contains(&rin.as_str()) {
            return true;
        } else if ["n", "no", "nein"].contains(&rin.as_str()) {
            return false;
        }
        print!("couldn't parse that, please try again [y/n]: ");
    }
    println!("probably not");
    false
}

fn write_text_marks<P: AsRef<std::path::Path>>(
    peaks: &[Peak<SampleType>],
    sr: SampleType,
    path: P,
    in_between: Duration,
    dry_run: bool,
) -> Result<(), CliError> {
    let mut out = String::new();
    for (i, (start, end)) in peaks
        .iter()
        .map(|p| p.position.start as SampleType / sr)
        .tuple_windows()
        .enumerate()
    {
        out += (start as f64 + in_between.as_secs_f64())
            .to_string()
            .as_str();
        out.push('\t');
        out += (end).to_string().as_str();
        out.push_str("\tSegment ");
        out += (i + 1).to_string().as_str();
        out.push('\n')
    }

    if dry_run {
        info(&format!(
            "writing \"\"\"\n{out}\"\"\" > {}",
            path.as_ref().display()
        ));
    } else {
        std::fs::write(&path, out).map_err(|_| errors::CliError::CantCreateFile(path.into()))?;
    }
    Ok(())
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
