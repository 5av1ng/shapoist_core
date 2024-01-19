use crate::system::command::Command;
use crate::system::command::CommandError;
use crate::system::ChartError;
use crate::system::core_structs::Chart;
#[cfg(not(target_arch = "wasm32"))]
use kira::sound::static_sound::StaticSoundData;
#[cfg(not(target_arch = "wasm32"))]
use kira::sound::static_sound::StaticSoundSettings;
use crate::system::core_structs::ScriptInfo;
use crate::system::core_structs::Condition;
use std::ffi::OsStr;
use crate::system::core_structs::ChartInfo;
use crate::system::core_structs::NetWork;
use std::collections::HashMap;
use crate::system::timer::Timer;
use crate::system::core_structs::Settings;
use crate::system::Error::IoError;
use crate::system::Error;
use crate::system::io_functions::*;
use std::path::PathBuf;
use crate::system::core_structs::ShapoistCore;
use log::*;

impl ShapoistCore {
	/// create a new shapoist core
	/// 
	/// we will also do self check here
	/// note: the path you have given should **NOT** contain "shapoist_assets"
	#[cfg(not(target_arch="wasm32"))]
	pub fn new(assets_path: &str) -> Result<Self, Error> {
		debug!("creating new ShapoistCore struct..");
		info!("checking initlizaition infomation");
		match read_every_file(format!("{}/shapoist_assets", assets_path)) {
			Ok(_) => {},
			Err(e) => {
				if let Error::IoError(t) = e {
					if let std::io::ErrorKind::NotFound = t.kind() {
						return ShapoistCore::init(assets_path);
					}
				}else {
					return Err(e)
				}
			},
		};

		let settings = Settings::process_setting_file(assets_path)?;
		let log_path = PathBuf::from(process_log_path(assets_path, &settings)?);
		let mut chart_list = process_chart_path(assets_path)?;
		let mut script_list = process_script_path(assets_path)?;

		if settings.need_check_chart {
			for chart in &mut chart_list {
				let _ = chart.check();
			}
		}

		if settings.need_check_script {
			for script in &mut script_list {
				let _ = script.check();
			}
		}

		Ok(Self {
			log_path,
			settings,
			chart_list,
			script_list,
			..Default::default()
		})
	}

	#[cfg(target_arch="wasm32")]
	pub fn new(_: &str) -> Result<Self, Error> {
		debug!("creating new ShapoistCore struct..");
		info!("running in web mode, using minimal setup.");

		Ok(Self::default())
	}

	// /// running shapoist in terminal mode. would be a completely mess if you try to play in this mode.
	// pub fn run_terminal_mode(&mut self) -> Result<(), Error> {
	// 	info!("running in terminal mode");
	// 	loop {
	// 		let mut input = String::new();
	// 		if let Err(e) = std::io::stdin().read_line(&mut input) {
	// 			error!("reading line failed. info: {}", e);
	// 			return Err(e.into())
	// 		}
	// 		if input.trim() == String::from("exit") {
	// 			break;
	// 		}
	// 		self.parse_command(input)?;
	// 	}
	// 	Ok(())
	// }

	/// parse given command, shouldn't take too much time
	pub fn parse_command(&mut self, command: Command) -> Result<(), Error> {
		debug!("parsing command: {:?}", command);
		command.parse(self)?;
		if !self.command_history.is_empty() {
			if command != self.command_history[self.command_history.len() - 1] {
				self.command_history.push(command)
			}
		}
		if self.command_history.len() > self.settings.command_history {
			self.command_history.remove(0);
		}
		Ok(())
	}

	/// call this function every frame your app updates is required
	pub fn frame(&mut self) -> Result<(), Error> {
		for index in 0..self.thread_pool.len() {
			if self.thread_pool[index].is_finished() {
				let thread = self.thread_pool.remove(index);
				// normally we will not panic in every thread..., if if thread panics, we will deliver a unknown error.
				match thread.join() {
					Ok(t) => {
						match t {
							Ok(_) => return Ok(()),
							Err(e) => return Err(CommandError::NetworkError(e).into())
						}
					},
					Err(_) => return Err(Error::Unknown)
				}
			}
		}
		Ok(())
	}

	/// if we didn't have our shapoist_assets floor, we will create one
	#[allow(dead_code)]
	fn init(assets_path: &str) -> Result<Self, Error> {
		info!("{}/shapoist_assets doesn't found, initlizing...", assets_path);
		let _ = remove_path(format!("{}/shapoist_assets", assets_path));
		create_dir(format!("{}/shapoist_assets", assets_path))?;

		let settings_path = format!("{}/shapoist_assets/settings.toml", assets_path);
		let log_path = format!("{}/shapoist_assets/log", assets_path);
		let log_name = log_name_generate(assets_path);
		let chart_path = format!("{}/shapoist_assets/chart", assets_path);
		let user_path = format!("{}/shapoist_assets/user", assets_path);
		let script_path = format!("{}/shapoist_assets/script", assets_path);

		create_file(settings_path.clone())?;
		write_file(settings_path, to_toml(&Settings::default())?.as_bytes())?;
		create_dir(log_path)?;
		create_file(log_name.clone())?;
		create_dir(chart_path)?;
		create_dir(user_path)?;
		create_dir(script_path)?;
		Ok(Self {
			log_path: PathBuf::from(log_name),
			..Default::default()
		})
	}
}

#[allow(dead_code)]
fn log_name_generate(assets_path: &str) -> String {
	let fmt = "%Y-%m-%d %H%M%S";
	let now = chrono::Local::now().format(fmt).to_string();
	format!("{}/shapoist_assets/log/[{}]running.log", assets_path, now)
}

impl Settings {
	/// read and process setting file
	pub fn process_setting_file(assets_path: &str) -> Result<Self, Error> {
		info!("processing settings file");
		let settings_path = format!("{}/shapoist_assets/settings.toml", assets_path);
		let settings = parse_toml(&match read_file_to_string(settings_path.clone()){
			Ok(t) => t,
			Err(IoError(t)) => {
				if let std::io::ErrorKind::NotFound = t.kind() {
					warn!("settings file not found, using default settings instead.");
					create_file(settings_path.clone())?;
					write_file(settings_path, to_toml(&Settings::default())?.as_bytes())?;
					to_toml(&Settings::default())?
				}else {
					return Err(t.into())
				}
			},
			Err(e) => return Err(e),
		})?;

		Ok(settings)
	}
}

impl ChartInfo {
	/// read and process chart from path
	pub fn process(path: &str) -> Result<ChartInfo, Error> {
		info!("processing single chart");
		return Ok(ChartInfo {
			path: PathBuf::from(path),
			// since we haven't done a fully check, so we will remain condition as Unknown.
			condition: Condition::Unknown,
			..parse_toml(&read_file_to_string(format!("{}/config.toml",path))?)?
		})
	}

	/// check if this chart is avaliable, returns the reason why it is broken
	pub fn check(&mut self) -> Result<(), Error> {
		if let Condition::Normal = self.condition {
			return Ok(())
		}

		self.condition = Condition::Broken;
		let is_file_missing = !(PathBuf::from(format!("{}/song.mp3",self.path.display())).exists() &&
			PathBuf::from(format!("{}/back.png",self.path.display())).exists() &&
			PathBuf::from(format!("{}/chart.scf",self.path.display())).exists() &&
			PathBuf::from(format!("{}/config.toml",self.path.display())).exists());
		if is_file_missing {
			return Err(ChartError::FileMissing.into());
		}

		let chart = match read_file_to_string(format!("{}/chart.scf", self.path.display())) {
			Ok(t) => t,
			Err(_) => {
				return Err(ChartError::UnsupportedEncoding.into())
			}
		};
		match parse_toml::<Chart>(&chart) {
			Ok(_) => {},
			Err(_) => {
				return Err(ChartError::CantParseChart.into())
			}
		}
		#[cfg(not(target_arch = "wasm32"))]
		match StaticSoundData::from_file(format!("{}/song.mp3", self.path.display()), StaticSoundSettings::default()) {
		    Ok(_) => {}
		    Err(e) => return Err(ChartError::MusicSourceCantRead(e).into()),
		}

		self.condition = Condition::Normal;
		Ok(())
	}
}

impl ScriptInfo {
	/// read and process script from path
	pub fn process(path: &str) -> Result<ScriptInfo, Error> {
		info!("processing single script");
		let is_broken = !PathBuf::from(format!("{}/config.toml",path)).exists();
		if is_broken {
			warn!("script in {} has broken", path);
			return Ok(ScriptInfo {
				condition: Condition::Broken,
				path: PathBuf::from(path),
				..Default::default()
			})
		}else {
			return Ok(ScriptInfo {
				path: PathBuf::from(path),
				// since we haven't done a fully check, so we will remain condition as Unknown.
				condition: Condition::Unknown,
				..parse_toml(&read_file_to_string(format!("{}/config.toml",path))?)?
			})
		}
	}

	/// check if this script is avaliable, returns the reason why it is broken
	pub fn check(&mut self) -> Result<(), Error> {
		todo!();
	}
}

#[allow(dead_code)]
fn process_log_path(assets_path: &str, settings: &Settings) -> Result<String, Error> {
	info!("processing log path");
	let log_path = format!("{}/shapoist_assets/log", assets_path);
	let log_name = log_name_generate(assets_path);
	for log_file in read_every_file(log_path)? {
		let metadata = read_metadata(log_file.clone())?;
		if let Ok(t) = match metadata.created() {
			Ok(t) => t,
			Err(e) => return Err(e.into())
		}.elapsed() {
			if t > settings.log_remain {
				remove_file(log_file)?
			};
		}
	}
	// create_file(log_name.clone())?;

	Ok(log_name)
}

#[allow(dead_code)]
fn process_chart_path(assets_path: &str) -> Result<Vec<ChartInfo>, Error> {
	info!("processing chart path");
	let chart_path = format!("{}/shapoist_assets/chart", assets_path);
	let mut charts = vec!();
	for chart in read_every_file(chart_path.clone())? {
		let metadata = read_metadata(chart.clone())?;
		if metadata.is_dir() {
			for detailed_chart in read_every_file(chart.clone())? {
				charts.push(ChartInfo::process(&detailed_chart)?);
			}
		}
		if metadata.is_file() {
			let supported_extension = vec!(OsStr::new("scc"));
			let file = PathBuf::from(chart.clone());
			let extension = match file.extension() {
				Some(t) => t,
				None => OsStr::new(""),
			}; 
			if supported_extension.contains(&extension) {
				unzip(&chart, &chart_path)?;
				let mut path = PathBuf::from(chart.clone());
				path.set_extension("");
				let path = match path.to_str() {
					Some(t) => t,
					None => return Err(std::io::Error::from(std::io::ErrorKind::AddrNotAvailable).into()),
				};
				charts.push(ChartInfo::process(&path)?)
			}
		}
	}
	Ok(charts)
}

#[allow(dead_code)]
fn process_script_path(assets_path: &str) -> Result<Vec<ScriptInfo>, Error> {
	info!("processing script path");
	let script_path = format!("{}/shapoist_assets/script", assets_path);
	let mut scripts = vec!();
	for script in read_every_file(script_path.clone())? {
		let metadata = read_metadata(script.clone())?;
		if metadata.is_dir() {
			for detailed_script in read_every_file(script.clone())? {
				scripts.push(ScriptInfo::process(&detailed_script)?);
			}
		}
		if metadata.is_file() {
			let supported_extension = vec!(OsStr::new("ssc"));
			let file = PathBuf::from(script.clone());
			let extension = match file.extension() {
				Some(t) => t,
				None => OsStr::new(""),
			}; 
			if supported_extension.contains(&extension) {
				unzip(&script, &script_path)?;
				let mut path = PathBuf::from(script.clone());
				path.set_extension("");
				let path = match path.to_str() {
					Some(t) => t,
					None => return Err(std::io::Error::from(std::io::ErrorKind::AddrNotAvailable).into()),
				};
				scripts.push(ScriptInfo::process(path)?);
			}
		}
	}
	Ok(scripts)
}

#[allow(dead_code)]
fn unzip(path: &str, uzip_path: &str) -> Result<(), Error> {
	info!("unziping file {}", path);
	let shapoist_compress = read_file(path)?;
	let mut zip = zip::ZipArchive::new(shapoist_compress)?;
	for i in 0..zip.len() {
		let file = zip.by_index(i)?;
		write_file(format!("{}/{}" ,uzip_path, file.mangled_name().display()), file)?;
	}

	Ok(())
}

impl Default for ShapoistCore {
	fn default() -> Self {
		Self {
			log_path: PathBuf::new(),
			chart_list: Vec::new(),
			current_chart: None,
			chart_editor: None,
			script_list: Vec::new(),
			temp: HashMap::new(),
			network: NetWork::default(),
			play_info: None,
			judge_event: Vec::new(),
			timer: Timer::default(),
			command_history: Vec::new(),
			settings: Settings::default(),
			thread_pool: Vec::new(),
		}
	}
}