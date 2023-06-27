use puremp3::*;
use itertools::Itertools;

fn main() {
    let path = "res/Interlude.mp3";
    let file = std::fs::File::open(path).expect("couldn't find file");
    let (header, samples) = read_mp3(file).expect("Invalid MP3");

    let samples: Vec<(f32, f32)> = samples.collect_vec();
    let sr: Option<u32> = Some(header.sample_rate.hz());
    let ch: Option<usize> = Some(header.channels.num_channels());

    // let max = samples.iter().max_by(|a, b| a.total_cmp(b)).unwrap();
    println!("#{} over #{} channels at {} with max of ?", samples.len(), ch.unwrap(), sr.unwrap());
    println!("starts with {:?}", &samples[samples.len()-500..]);
}
