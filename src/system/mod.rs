//! Some shapoist system types and functions

mod core_functions;
mod io_functions;

pub mod core_structs;
pub mod timer;
pub mod command;

use crate::system::command::CommandError;
use crate::system::timer::TimerError;

#[derive(thiserror::Error, Debug, Default)]
/// every types of errors in shapoist
pub enum Error {
	/// represent some errors that shouldn't happen
	#[error("Unknown Error")]
	#[default] Unknown,
	/// errors when using [`timer::Timer`]
	#[error("Timer Error")]
	TimerError(#[from] TimerError),
	/// errors when doing io functions
	#[error("I/O Error")]
	IoError(#[from] std::io::Error),
	/// errors when converting into toml
	#[error("Serdelize Error")]
	ConvertToError(#[from] toml::ser::Error),
	/// errors when converting back form toml
	#[error("Deserdelize Error")]
	ConvertInError(#[from] toml::de::Error),
	/// possible reasons of why a chart is not usable.
	#[error("Chart Parsing Error")]
	ChartError(#[from] ChartError),
	/// possible reasons during using zip
	#[error("Zip Error")]
	ZipError(#[from] zip::result::ZipError),
	#[error("Command Error")]
	CommannError(#[from] CommandError),
}

#[non_exhaustive]
/// possible reasons of why a chart is not usable.
#[derive(thiserror::Error, Debug)]
pub enum ChartError {
	#[error("Missing files")]
	FileMissing,
	#[error("Unsupported Encoding")]
	UnsupportedEncoding,
	#[error("Cant parse Chart")]
	CantParseChart,
	#[error("Cant read music source")]
	MusicSourceCantRead(#[from] kira::sound::FromFileError)
}
