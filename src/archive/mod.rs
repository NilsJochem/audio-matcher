use log::debug;

use self::data::Archive;

pub mod args;
pub mod data;

pub fn run(args: &self::args::Arguments) -> Result<(), crate::matcher::errors::CliError> {
    debug!("{args:#?}");

    let archive = Archive::read(&args.path).unwrap();
    let mut s = String::new();
    archive.format(&mut s, "\t", false, true).unwrap();
    println!("{s}");
    Ok(())
}
