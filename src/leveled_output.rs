pub static mut OUTPUT_LEVEL: OutputLevel = OutputLevel::Info;
#[derive(PartialEq, PartialOrd)]
pub enum OutputLevel {
    Debug,
    Verbose,
    Info,
    Error,
}

pub(crate) fn println(level: &OutputLevel, msg: &dyn AsRef<str>) {
    if unsafe { OUTPUT_LEVEL <= *level } {
        if *level == OutputLevel::Error {
            eprintln!("{}", msg.as_ref())
        } else {
            println!("{}", msg.as_ref())
        }
    }
}
pub(crate) fn print(level: &OutputLevel, msg: &dyn AsRef<str>) {
    if unsafe { OUTPUT_LEVEL <= *level } {
        if *level == OutputLevel::Error {
            eprint!("{}", msg.as_ref())
        } else {
            print!("{}", msg.as_ref())
        }
    }
}

#[inline]
pub fn error(msg: &dyn AsRef<str>) {
    println(&OutputLevel::Error, msg);
}
#[inline]
pub fn info(msg: &dyn AsRef<str>) {
    println(&OutputLevel::Info, msg);
}
#[inline]
pub fn verbose(msg: &dyn AsRef<str>) {
    println(&OutputLevel::Verbose, msg);
}
#[inline]
pub fn debug(msg: &dyn AsRef<str>) {
    println(&OutputLevel::Debug, msg);
}
