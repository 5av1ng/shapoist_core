use shapoist_request::prelude::*;
use crate::system::core_structs::ShapoistCore;
use shapoist_request::prelude::RequestCommand;

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
/// a command enum
pub enum Command {
	NetworkCommand(RequestCommand)
}

#[derive(thiserror::Error, Debug, Default)]
pub enum CommandError{
	/// represent some errors that shouldn't happen
	#[error("Unknown Error")]
	#[default] Unknown,
	/// happens when [`ShapoistCore`] run out thread pool
	#[error("Running Out Thread Pool")]
	RunOutThreadPool,
	#[error("Network Error")]
	NetworkError(#[from] ClientError)
}

impl Command {
	pub(crate) fn parse(&self, core: &mut ShapoistCore) -> Result<(), CommandError> {
		match self {
			Self::NetworkCommand(rc) => {
				if core.thread_pool.len() > core.settings.thread_handels {
					return Err(CommandError::RunOutThreadPool)
				}else {
					core.thread_pool.push(core.network.request.send(rc))
				}
				
			}
		}
		Ok(())
	}
}