use self::data::Archive;


pub mod data;
pub mod args;


pub fn run (args: &self::args::Arguments) -> Result<(), crate::matcher::errors::CliError> {
	let archive = Archive::read(&args.path).unwrap();
	let mut s = String::new();
	archive.format(&mut s, "\t", false, true).unwrap();
	println!("{s}");
	Ok(())
}
