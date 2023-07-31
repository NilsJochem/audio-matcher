use log::debug;

use self::data::Archive;

pub mod args;
pub mod data;

pub fn run(args: &self::args::Arguments) -> Result<(), crate::matcher::errors::CliError> {
    debug!("{args:#?}");

    let archive = Archive::read(args.archive.as_ref().unwrap());
    println!("{}", archive.as_display("\t", false, true));
    Ok(())
}
