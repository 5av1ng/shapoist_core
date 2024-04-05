//! a simple timer

use time::OffsetDateTime;
use time::Duration;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
#[serde(default)]
/// a simple timer
pub struct Timer {
	/// when timer start, if start_time > pause_time then its satred
	pub start_time: OffsetDateTime,
	/// when timer pasue, if pause_time > start_time then its paused
	pub pause_time: OffsetDateTime
}

impl Timer {
	/// create a new timer
	pub fn new() -> Self {
		let start_time = OffsetDateTime::now_utc();
		let pause_time = OffsetDateTime::now_utc();
		Self {
			start_time,
			pause_time,
		}
	}

	/// reset the timer
	pub fn reset(&mut self) {
		let start_time = OffsetDateTime::now_utc();
		let pause_time = OffsetDateTime::now_utc();
		*self = Self {
			start_time,
			pause_time,
		}
	}

	/// check if a timer is started
	pub fn is_started(&self) -> bool {
		self.start_time >= self.pause_time
	}

	/// read the timer
	pub fn read(&self) -> Duration {
		if self.is_started() {
			OffsetDateTime::now_utc() - self.start_time
		}else {
			self.pause_time - self.start_time
		}
	}

	/// pause the timer, will do nothing if timer has paused
	pub fn pause(&mut self){
		if self.is_started() {
			self.pause_time = OffsetDateTime::now_utc();
		}
	}

	/// start the timer, will do nothing if timer has started
	pub fn start(&mut self) {
		let read = self.read();
		if !self.is_started() {
			self.pause_time = OffsetDateTime::now_utc() - read - Duration::seconds(2);
			self.start_time = OffsetDateTime::now_utc() - read;
		};
	}

	/// set the timer, positive duration means earlier, read should be larger
	pub fn set(&mut self, offset: Duration) {
		if self.is_started() {
			self.pause_time = self.pause_time - offset;
			self.start_time = self.start_time - offset;
		}else {
			self.pause_time = self.start_time + offset;
		}
	}

	/// set the timer to exactly where you give.
	pub fn set_to(&mut self, pointer: Duration){
		if self.is_started() {
			self.pause_time = OffsetDateTime::now_utc() - pointer;
			self.start_time = OffsetDateTime::now_utc() - pointer;
		}else {
			self.pause_time = self.start_time + pointer;
		}
	}
}

impl Default for Timer {
	fn default() -> Self {
		Self::new()
	}
}

impl Copy for Timer {}