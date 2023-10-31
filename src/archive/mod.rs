use std::{path::PathBuf, str::FromStr};

use clap::{Parser, Subcommand};
use log::{debug, warn};
use shellwords::MismatchedQuotes;
use thiserror::Error;

use crate::{
    args::{autocompleter::VecCompleter, Inputs},
    worker::ChapterCompleter,
};

use self::data::Archive;

pub mod args;
pub mod data;

pub fn run(args: &self::args::Arguments) -> Result<(), crate::matcher::errors::CliError> {
    debug!("{args:#?}");
    let mut holder = Holder::new(args.archive.as_ref().unwrap().clone());

    if args.interactive {
        holder.work_commands(CommandReader::default());
    } else {
        holder.work_commands(std::iter::once(Command::List {
            indent: "\t".to_owned(),
            print_all: true,
        }));
    }
    Ok(())
}
struct Holder {
    archive: Archive,
    path: PathBuf,
}
impl Holder {
    fn new(path: PathBuf) -> Self {
        Self {
            archive: Archive::read(&path),
            path,
        }
    }
    fn work_commands(&mut self, iter: impl Iterator<Item = Command>) {
        for command in iter {
            debug!("processsing {command:?}");
            match command {
                Command::Exit => break,
                Command::Help => {} // Cli::command_for_update().print_long_help().unwrap(),
                Command::Reload { path } => {
                    self.archive = Archive::read(path.as_deref().unwrap_or(&self.path));
                }
                Command::List { indent, print_all } => {
                    println!("{}", self.archive.as_display(&indent, false, print_all));
                }
                Command::Rename => println!("comming soon"),
            }
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "", arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Exit,
    Help,
    Reload {
        path: Option<PathBuf>,
    },
    List {
        #[clap(default_value_t = String::from("/t"))]
        indent: String,
        /// should chapters be printed
        #[clap(name = "print_chapters", short = 'c', long)]
        print_all: bool,
    },
    Rename,
}
#[derive(Debug, Error)]
#[error(transparent)]
pub enum Error {
    MismatchedQuotes(#[from] MismatchedQuotes),
    Parse(#[from] clap::Error),
}
impl FromStr for Command {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let words = shellwords::split(s)?;
        Ok(Cli::try_parse_from(std::iter::once(String::new()).chain(words))?.command)
    }
}
#[derive(Debug, Default)]
pub struct CommandReader {
    is_finnished: bool,
}
impl Iterator for CommandReader {
    type Item = Command;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_finnished {
            return None;
        }
        let command = Inputs::map_input("$> ", Command::Help, None::<&str>, |input| match input
            .parse::<Command>(
        ) {
            Ok(command) => Some(command),
            Err(err) => {
                if !input.is_empty() {
                    warn!("{err}");
                }
                None
            }
        });

        if matches!(command, Command::Exit) {
            debug!("read Exit, stoping read");
            self.is_finnished = true;
        } else {
            log::trace!("read {command:?}");
        };
        Some(command)
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct CliCompleter<'a> {
    archive: &'a Archive,
    series_completer: VecCompleter,
    chapter_completer: Option<ChapterCompleter<'a>>,
    filter: Box<dyn crate::args::autocompleter::StrFilter + Send + Sync>,
}
impl<'a> crate::args::autocompleter::MyAutocomplete for CliCompleter<'a> {
    fn get_suggestions(&mut self, _input: &str) -> Result<Vec<String>, inquire::CustomUserError> {
        todo!()
    }

    fn get_completion(
        &mut self,
        _input: &str,
        _highlighted_suggestion: Option<String>,
    ) -> Result<inquire::autocompletion::Replacement, inquire::CustomUserError> {
        todo!()
    }
}
