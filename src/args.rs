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
    pub fn ask_consent(&self, msg: &str) -> bool {
        if self.yes || self.no {
            return self.yes;
        }
        print!("{msg} [y/n]: ");
        for _ in 0..self.trys {
            let rin: String = text_io::read!("{}\n");
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
