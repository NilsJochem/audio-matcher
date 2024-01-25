pub mod archive;
pub mod args;
pub mod matcher;
pub mod worker;

pub const APP_NAME: &str = "audio-matcher"; // on change remember to change value in audacity

const fn offset_range(range: &std::ops::Range<usize>, offset: usize) -> std::ops::Range<usize> {
    (range.start + offset)..(range.end + offset)
}
