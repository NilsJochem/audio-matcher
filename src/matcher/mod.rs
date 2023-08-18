pub mod args;
#[allow(clippy::module_name_repetitions)] // TODO fix
pub mod audio_matcher;
pub mod errors;
pub mod mp3_reader;

use std::time::Duration;

use crate::{archive::data::timelabel_from_peaks, iter::IteratorExt};
use audacity::data::TimeLabel;
use errors::CliError;
use log::{debug, info, log, trace};

use mp3_reader::SampleType;

pub fn run(args: &args::Arguments) -> Result<(), CliError> {
    debug!("{args:#?}");

    if args.out_file.out_file.is_some() {
        assert_eq!(
            1,
            args.within.len(),
            "providet outfile only compatible with one main file"
        );
    }

    trace!("collecting snippet data");
    let (sr, s_samples) = mp3_reader::read_mp3(&args.snippet)?;
    let s_duration = mp3_reader::mp3_duration(&args.snippet, false)?;

    let sample_data = s_samples.collect::<Box<[SampleType]>>();
    trace!("preparing algo");
    let algo = audio_matcher::LibConvolve::new(sample_data);
    let level = if args.within.len() == 1 {
        // log number of iterations only if more than one file is processed
        log::Level::Trace
    } else {
        log::Level::Info
    };

    for main_file in &args.within {
        let out_path = args
            .out_file
            .out_file
            .clone()
            .or_else(|| (!args.out_file.no_out).then(|| auto_out_file(main_file)));
        let out_path = if out_path.as_ref().is_some_and(|path| path.exists()) {
            if args.skip_existing
                || args.always_answer.ask_consent(format!(
                    "Ausgabe Datei {:?} existiert bereits, m\u{f6}chtest du skippen",
                    out_path
                        .as_ref()
                        .and_then(|it| it.file_name())
                        .unwrap_or_else(|| panic!("couldn't get filename of {out_path:?}"))
                ))
            {
                continue;
            }
            out_path.filter(|_| {
                args.always_answer
                    .ask_consent("soll die existierende Datei \u{fc}berschrieben werden")
            })
        } else {
            out_path
        };

        // TODO only fail this loop iteration
        log!(level, "preparing data of '{}'", main_file.display());

        let (m_sr, m_samples) = mp3_reader::read_mp3(&main_file)?;
        if sr != m_sr {
            return Err(errors::CliError::SampleRateMismatch(sr, m_sr));
        }

        trace!("collecting main duration");
        let m_duration = mp3_reader::mp3_duration(main_file, false)?;
        trace!("calculation chunks");
        let peaks = audio_matcher::calc_chunks(
            sr,
            m_samples.with_size((m_duration.as_secs_f64() * sr as f64) as usize),
            &algo,
            true,
            audio_matcher::Config::from_args(args, s_duration),
        );

        print_offsets(&peaks, sr);
        debug!("found peaks {:#?}", &peaks);

        if let Some(out_path) = out_path {
            trace!("writing result to '{}'", out_path.display());
            TimeLabel::write(
                timelabel_from_peaks(peaks.iter(), sr, Duration::from_secs(7), "Segment #"),
                &out_path,
                args.dry_run,
            )
            .map_err(|_| CliError::NoFile(out_path.into()))?;
        }
    }

    Ok(())
}

fn auto_out_file(path: impl AsRef<std::path::Path>) -> std::path::PathBuf {
    path.as_ref().with_extension("txt")
}

fn print_offsets(peaks: &[find_peaks::Peak<SampleType>], sr: u16) {
    if peaks.is_empty() {
        info!("no offsets found");
    }
    for (i, peak) in peaks.iter().enumerate() {
        let (hours, minutes, seconds) = crate::split_duration(&start_as_duration(peak, sr));
        info!(
            "Offset {}: {:0>2}:{:0>2}:{:0>2} with prominence {}",
            i + 1,
            hours,
            minutes,
            seconds,
            &peak.prominence.unwrap()
        );
    }
}

pub(crate) fn start_as_duration(peak: &find_peaks::Peak<SampleType>, sr: u16) -> Duration {
    Duration::from_secs_f64(peak.position.start as f64 / sr as f64)
}
