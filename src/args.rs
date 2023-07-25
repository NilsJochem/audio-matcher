use clap::Args;

#[derive(Args, Debug, Clone, Copy)]
#[group(required = false, multiple = false)]
pub struct Inputs {
    #[clap(short, help = "always answer yes")]
    pub yes: bool,
    #[clap(short, help = "always answer no")]
    pub no: bool,
    #[clap(long, default_value_t = 3, help = "number of retrys")]
    pub trys: u8,
}
impl Inputs {
    #[must_use]
    pub const fn test() -> Self {
        Self {
            yes: false,
            no: false,
            trys: 3,
        }
    }
    #[must_use]
    pub fn ask_consent(&self, msg: &str) -> bool {
        if self.yes || self.no {
            return self.yes;
        }
        self.try_input(&format!("{msg} [y/n]: "), None, |rin| {
            if ["y", "yes", "j", "ja"].contains(&rin.as_str()) {
                return Some(true);
            } else if ["n", "no", "nein"].contains(&rin.as_str()) {
                return Some(false);
            }
            None
        })
        .unwrap_or_else(|| {
            println!("probably not");
            false
        })
    }

    pub fn try_input<T>(
        &self,
        msg: &str,
        default: Option<T>,
        mut map: impl FnMut(String) -> Option<T>,
    ) -> Option<T> {
        print!("{msg}");
        for _ in 0..self.trys {
            let rin: String = text_io::read!("{}\n");
            if default.is_some() && rin.is_empty() {
                return default;
            }
            match map(rin) {
                Some(t) => return Some(t),
                None => print!("couldn't parse that, please try again: "),
            }
        }
        None
    }
    #[must_use]
    pub fn input(&self, msg: &str, default: Option<String>) -> String {
        self.try_input(msg, default, Some)
            .unwrap_or_else(|| unreachable!())
    }
}

#[derive(Args, Debug, Clone, Copy)]
#[group(required = false, multiple = false)]
pub struct OutputLevel {
    #[clap(short, long, help = "print maximum info")]
    debug: bool,
    #[clap(short, long, help = "print more info")]
    verbose: bool,
    #[clap(short, long, help = "print less info")]
    silent: bool,
}

impl From<OutputLevel> for crate::leveled_output::OutputLevel {
    fn from(val: OutputLevel) -> Self {
        if val.silent {
            Self::Error
        } else if val.verbose {
            Self::Verbose
        } else if val.debug {
            Self::Debug
        } else {
            Self::Info
        }
    }
}
