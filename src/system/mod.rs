//! Some shapoist system types and functions

mod core_functions;
mod io_functions;

pub mod core_structs;
pub mod timer;
pub mod command;

use crate::system::command::CommandError;

#[derive(thiserror::Error, Debug, Default)]
/// every types of errors in shapoist
pub enum Error {
	/// represent some errors that shouldn't happen
	#[error("Unknown Error")]
	#[default] Unknown,
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
	#[error("Play Error, info: {0}")]
	PlayError(#[from] PlayError),
	#[error("Chart Edit Error, info: {0}")]
	ChartEditError(#[from] ChartEditError),
	#[error("the funtion you call is not available on {0} platform")]
	PlatformUnsupport(String)
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
	#[error("Cant read music source, info: {0}")]
	MusicSourceCantRead(#[from] kira::sound::FromFileError),
	#[error("Cant read music source, info: {0}")]
	MusicSourceCantReadString(String)
}

#[non_exhaustive]
/// possible reasons that will appear when playing chart or pause chart
#[derive(thiserror::Error, Debug)]
pub enum PlayError {
	#[error("no chart loaded")]
	NoChartLoaded,
	#[error("haven't start play")]
	HaventStart,
	#[error("manager create failed, info: {0}")]
	ManagerCreateFail(#[from] kira::manager::backend::cpal::Error),
	#[error("clock create failed, info: {0}")]
	ClockCreateFailed(#[from] kira::manager::error::AddClockError),
	#[error("music play failed, info: {0}")]
	MusicPlayFailed(#[from] kira::manager::error::PlaySoundError<()>),
	#[error("error during using kira, info: {0}")]
	KiraError(#[from] kira::CommandError),
}

#[non_exhaustive]
/// possible reasons that will appear when creating a chart
#[derive(thiserror::Error, Debug)]
pub enum ChartEditError {
	#[error("missing info")]
	MissingInfo,
	#[error("not in edit mode")]
	NotInEditMode,
}