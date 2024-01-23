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
	#[error("Timer Error, info: {0}")]
	TimerError(#[from] TimerError),
	/// errors when doing io functions
	#[error("I/O Error, info: {0}")]
	IoError(#[from] std::io::Error),
	/// errors when converting into toml
	#[error("Serdelize Error, info: {0}")]
	ConvertToError(#[from] toml::ser::Error),
	/// errors when converting back form toml
	#[error("Deserdelize Error, info: {0}")]
	ConvertInError(#[from] toml::de::Error),
	/// possible reasons of why a chart is not usable.
	#[error("Chart Parsing Error, info: {0}")]
	ChartError(#[from] ChartError),
	/// possible reasons during using zip
	#[error("Zip Error, info: {0}")]
	ZipError(#[from] zip::result::ZipError),
	#[error("Command Error, info: {0}")]
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
