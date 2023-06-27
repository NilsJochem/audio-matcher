use minimp3::{Decoder, Frame, Error};
use itertools::Itertools;


fn main() {
    let path = "res/Interlude.mp3";
    let file = std::fs::File::open(path).expect("couldn't find file");
    let mut decoder = Decoder::new(file);

    let mut samples: Vec<(f64, f64)> = Vec::new();
    let mut sr: Option<i32> = None;
    let mut ch: Option<usize> = None;
    loop {
        match decoder.next_frame() {
            Ok(Frame { data, sample_rate, channels, .. }) => {
                samples.extend(data
                    .iter()
                    .map(|s| -> f64 { *s as f64 / 32768.0 })
                    .chunks(channels)
                    .into_iter()
                    .map(|c| ->(f64, f64) {c.collect_tuple().unwrap()})
                );
                match sr {
                    Some(old_sr) => if old_sr != sample_rate {
                        panic!("sample rate changed")
                    },
                    None => sr = Some(sample_rate),
                }
                match ch {
                    Some(old_channels) => if old_channels != channels {
                        panic!("sample rate changed")
                    },
                    None => ch = Some(channels),
                }
            },
            Err(Error::Eof) => break,
            Err(e) => panic!("{:?}", e),
        }
    }
    while samples.first().map_or(false, |(s1, s2)| -> bool{s1.abs() <= 1.0/ 32768.0 || s2.abs() <= 1.0/ 32768.0}) {
        samples.remove(0);
    }
    while samples.last().map_or(false, |(s1, s2)| -> bool{s1.abs() <= 1.0/ 32768.0 || s2.abs() <= 1.0/ 32768.0}) {
        samples.remove(samples.len()-1);
    }
    // let max = samples.iter().max_by(|a, b| a.total_cmp(b)).unwrap();
    println!("#{} over #{} channels at {} with max of ?", samples.len(), ch.unwrap(), sr.unwrap());
    println!("starts with {:?}", &samples[samples.len()-500..]);
}
