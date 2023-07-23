#![warn(
    clippy::nursery,
    clippy::pedantic,
    clippy::empty_structs_with_brackets,
    clippy::format_push_string,
    clippy::if_then_some_else_none,
    clippy::impl_trait_in_params,
    clippy::missing_assert_message,
    clippy::multiple_inherent_impl,
    clippy::non_ascii_literal,
    clippy::self_named_module_files,
    clippy::semicolon_inside_block,
    clippy::separated_literal_suffix,
    clippy::str_to_string,
    clippy::string_to_string
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate
)]

pub mod args;
pub mod audio_matcher;
mod data;
mod errors;
mod iter;
pub mod leveled_output;
pub mod mp3_reader;

use data::TimeLabel;
use errors::CliError;
use mp3_reader::SampleType;
use std::{time::Duration, usize};

const fn offset_range(range: &std::ops::Range<usize>, offset: usize) -> std::ops::Range<usize> {
    (range.start + offset)..(range.end + offset)
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

#[inline]
pub const fn split_duration(duration: &Duration) -> (usize, usize, usize) {
    let elapsed = duration.as_secs() as usize;
    let seconds = elapsed % 60;
    let minutes = (elapsed / 60) % 60;
    let hours = elapsed / 3600;
    (hours, minutes, seconds)
}

pub fn run(args: &args::Arguments) -> Result<(), CliError> {
    unsafe {
        crate::leveled_output::OUTPUT_LEVEL = args.output_level.into();
    }
    debug!("{args:#?}");

    if args.out_file.out_file.is_some() {
        assert_eq!(
            1,
            args.within.len(),
            "providet outfile only compatible with one main file"
        );
    }
    let loop_start_level = if args.within.len() == 1 {
        leveled_output::OutputLevel::Verbose
    } else {
        leveled_output::OutputLevel::Info
    };

    verbose!("collecting snippet data");
    let (sr, s_samples) = mp3_reader::read_mp3(&args.snippet)?;
    let s_duration = mp3_reader::mp3_duration(&args.snippet, false)?;

    let sample_data = s_samples.collect::<Box<[SampleType]>>();
    verbose!("preparing algo");
    let algo = audio_matcher::LibConvolve::new(sample_data);

    for main_file in &args.within {
        // TODO only fail this loop iteration
        println_log!(loop_start_level, "preparing data of '{}'", main_file.display());
        let (m_sr, m_samples) = mp3_reader::read_mp3(&main_file)?;
        if sr != m_sr {
            return Err(errors::CliError::SampleRateMismatch(sr, m_sr));
        }

        verbose!("collecting main duration");
        let m_duration = mp3_reader::mp3_duration(&main_file, false)?;
        verbose!("calculation chunks");
        let peaks = audio_matcher::calc_chunks(
            sr,
            m_samples,
            &algo,
            m_duration,
            true,
            audio_matcher::Config::from_args(args, s_duration),
        );

        print_offsets(&peaks, sr);
        debug!("found peaks {:#?}", &peaks);

        if let Some(out_path) = args
            .out_file
            .out_file
            .clone()
            .or_else(|| (!args.out_file.no_out).then(|| auto_out_file(main_file.clone())))
            .filter(|path| args.should_overwrite_if_exists(path))
        {
            verbose!("writing result to '{}'", out_path.display());
            TimeLabel::write_text_marks(
                TimeLabel::from_peaks(&peaks, sr, Duration::from_secs(7), "Segment #"),
                &out_path,
                args.dry_run,
            )?;
        }
    }

    Ok(())
}

fn auto_out_file(mut path: std::path::PathBuf) -> std::path::PathBuf {
    path.set_extension("txt");
    path
}
