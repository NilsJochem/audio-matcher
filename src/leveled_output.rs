#[derive(PartialEq, Eq, Ord, PartialOrd, Clone, Copy)]
pub enum OutputLevel {
    Debug,
    Verbose,
    Info,
    Error,
}
pub(crate) static mut OUTPUT_LEVEL: OutputLevel = OutputLevel::Info;
#[must_use]
pub fn is_level(level: OutputLevel) -> bool {
    unsafe { OUTPUT_LEVEL <= level }
}

#[macro_export]
macro_rules! println_log {
    ($level:path, $($arg:tt)*) => {
        if $crate::leveled_output::is_level($level) {
            if $level == $crate::leveled_output::OutputLevel::Error {
                eprintln!($($arg)*);
            } else {
                println!($($arg)*);
            }
        }
    };
}
#[macro_export]
macro_rules! print_log {
    ($level:path, $($arg:tt)*) => {
        if $crate::leveled_output::is_level($level) {
            if $level == $crate::leveled_output::OutputLevel::Error {
                eprint!($($arg)*);
            } else {
                print!($($arg)*);
            }
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        $crate::println_log!($crate::leveled_output::OutputLevel::Error, $($arg)*)
    }};
}
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        $crate::println_log!($crate::leveled_output::OutputLevel::Info, $($arg)*)
    }};
}
#[macro_export]
macro_rules! verbose {
    ($($arg:tt)*) => {{
        $crate::println_log!($crate::leveled_output::OutputLevel::Verbose, $($arg)*)
    }};
}
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{
        $crate::println_log!($crate::leveled_output::OutputLevel::Debug, $($arg)*)
    }};
}
