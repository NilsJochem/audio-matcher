pub mod args;
pub mod audio_matcher;
pub mod errors;
pub mod mp3_reader;

use std::time::Duration;

use errors::CliError;
use crate::{debug, verbose, println_log, info};
use crate::archive::data::TimeLabel;

use mp3_reader::SampleType;


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
        crate::leveled_output::OutputLevel::Verbose
    } else {
        crate::leveled_output::OutputLevel::Info
    };

    verbose!("collecting snippet data");
    let (sr, s_samples) = mp3_reader::read_mp3(&args.snippet)?;
    let s_duration = mp3_reader::mp3_duration(&args.snippet, false)?;

    let sample_data = s_samples.collect::<Box<[SampleType]>>();
    verbose!("preparing algo");
    let algo = audio_matcher::LibConvolve::new(sample_data);

    for main_file in &args.within {
        // TODO only fail this loop iteration
        println_log!(
            loop_start_level,
            "preparing data of '{}'",
            main_file.display()
        );
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
                TimeLabel::from_peaks(peaks.iter(), sr, Duration::from_secs(7), "Segment #"),
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
