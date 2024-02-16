//! a simple timer

use time::OffsetDateTime;
use time::Duration;
use crate::system::Error;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq, thiserror::Error)]
pub enum TimerError {
	#[error("run a running timer")]
	Running,
	#[error("pause a paused timer")]
	Paused,
	#[error("cant read timer because {0}")]
	CouldNotRead(String),
	#[error("cant set timer because {0}")]
	CouldNotSet(String)
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Timer {
	last_pause_time: Option<OffsetDateTime>,
	last_start_time: Option<OffsetDateTime>,
	if_paused:bool,
}

impl Default for Timer {
	fn default() -> Self {
		Self {
			last_pause_time: None,
			last_start_time: None,
			if_paused: true,
		}
	}
}

impl Timer {
	/// start the timer, returns error if the timer has started
	pub fn start(&mut self) -> Result<(),Error> {
		if self.if_paused {
			let time_read = read_time();
			let last_start_time = match self.last_start_time {
				Some(t) => {
					let pause = match self.last_pause_time {
						Some(t) => time_read - t,
						None => Duration::ZERO,
					};
					t + pause
				},
				None => time_read,
			};
			self.last_start_time = Some(last_start_time);
			self.if_paused = false;
			Ok(())
		}else {
			Err(Error::TimerError(TimerError::Running))
		}
	}

	/// pause the timer, returns error if the timer has paused
	pub fn pause(&mut self) -> Result<(),Error> {
		if !self.if_paused {
			let time_read = read_time();
			let last_pause_time = Some(time_read);
			self.if_paused = true;
			self.last_pause_time = last_pause_time;
			Ok(())
		}else {
			Err(Error::TimerError(TimerError::Paused))
		}
	}

	/// read the timer, returns error if the timer has not set correctly
	pub fn read(&self) -> Result<Duration,Error> {
		let time: Duration;
		if self.if_paused {
			let pause = match self.last_pause_time {
				Some(t) => t,
				None => return Err(Error::TimerError(TimerError::CouldNotRead(String::from("havn't paused yet"))))
			};
			let start = match self.last_start_time {
				Some(t) => t,
				None => return Err(Error::TimerError(TimerError::CouldNotRead(String::from("havn't started yet"))))
			};
			time = pause - start;
			Ok(time)
		}else {
			let now = read_time();
			let start = match self.last_start_time {
				Some(t) => t,
				None => return Err(Error::TimerError(TimerError::CouldNotRead(String::from("havn't started yet"))))
			};
			time = now - start;
			Ok(time) 
		}
	}

	/// set the timer, returns error if the timer has not paused
	pub fn set(&mut self, delay: Duration) -> Result<(),Error> {
		let last_start_time = match self.last_start_time {
			Some(t) => t - delay,
			None => return Err(Error::TimerError(TimerError::CouldNotSet(String::from("haven't paused"))))
		};
		self.last_start_time = Some(last_start_time);
		Ok(())
	}

	/// check if a timer is started
	pub fn is_started(&self) -> bool {
		!self.if_paused
	}
}

fn read_time() -> OffsetDateTime {
	OffsetDateTime::now_utc()
}

impl Copy for Timer {}