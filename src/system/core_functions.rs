use std::ops::RangeFrom;
use nablo_shape::prelude::Area;
use kira::tween::Tween;
use nablo_shape::prelude::shape_elements::Style;
use crate::system::ChartEditError;
use nablo_data::CanBeAnimated;
use time::Duration;
use kira::manager::AudioManager;
use kira::manager::backend::DefaultBackend;
use kira::manager::AudioManagerSettings;
use crate::system::PlayError;
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
use crate::system::core_structs::*;
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
			assets_path: assets_path.into(),
			log_path,
			settings,
			chart_list,
			script_list,
			..Default::default()
		})
	}

	/// if settings have changed you should call this
	pub fn settings_are_changed(&mut self) -> Result<(), Error> {
		let settings_path = format!("{}/shapoist_assets/settings.toml", self.assets_path.display());
		write_file(settings_path, to_toml(&self.settings)?.as_bytes())?;
		Ok(())
	}

	/// reload all resource
	pub fn reload_all(&mut self) -> Result<(), Error> {
		let assets_path = format!("{}/", self.assets_path.display());
		let settings = Settings::process_setting_file(&assets_path)?;
		let mut chart_list = process_chart_path(&assets_path)?;
		let mut script_list = process_script_path(&assets_path)?;

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

		self.settings = settings;
		self.chart_list = chart_list;
		self.script_list = script_list;
		Ok(())
	}

	/// read a chart file
	pub fn read_chart(&mut self, info: &mut ChartInfo) -> Result<(), Error> {
		info.check()?;
		let chart = match read_file_to_string(format!("{}/chart.sc", info.path.display())) {
			Ok(t) => t,
			Err(_) => {
				return Err(ChartError::UnsupportedEncoding.into())
			}
		};
		match parse_toml::<Chart>(&chart) {
			Ok(inner) => {
				self.current_chart = Some((inner, info.clone()));
				Ok(())
			},
			Err(_) => {
				return Err(ChartError::CantParseChart.into())
			}
		}
	} 

	#[cfg(target_arch="wasm32")]
	pub fn new(_: &str) -> Result<Self, Error> {
		debug!("creating new ShapoistCore struct..");
		info!("running in web mode, using minimal setup.");
		// web user should be able to get their setting by login.

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
		trace!("running new frame, checking threads");
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

		if self.play_info.is_some() && self.timer.is_started() {
			trace!("detected is play a chart, updating each shape...");
			let play_info = self.play_info.as_mut().unwrap();
			let time = self.timer.read()?;
			if play_info.is_finished {
				if let Err(e) = play_info.audio_manager.pause(Tween {
					duration: std::time::Duration::from_secs(1),
					..Default::default()
				}) {
					return Err(PlayError::from(e).into());
				}
			}
			if !play_info.is_finished {
				let play_time = time + play_info.offcet + self.settings.offcet - Duration::seconds(3);
				if !play_info.is_track_played && play_time > Duration::ZERO {
					info!("music playing...");
					let sound_setting = StaticSoundSettings::new().playback_region(RangeFrom{ start: play_time.as_seconds_f64() });
					let static_sound = match StaticSoundData::from_file(&play_info.track_path, sound_setting){
						Ok(t) => t,
						Err(e) => return Err(ChartError::from(e).into()),
					};
					if let Err(e) = play_info.audio_manager.play(static_sound) {
						return Err(PlayError::from(e).into());
					};

					play_info.is_track_played = true;
					info!("music played");
				}
			}

			if !play_info.is_finished && time > Duration::seconds(3) {
				let time = time - Duration::seconds(3);
				

				if time > play_info.sustain_time {
					play_info.is_finished = true;
					play_info.shapes.clear();
					play_info.notes.clear();
					play_info.judge_fields.clear();
					play_info.click_effects.clear();
				};
				for i in play_info.current_render..play_info.shapes.len() {
					if play_info.shapes[i].start_time < time {
						play_info.render_queue.push(play_info.shapes[i].clone());
					}
					if play_info.shapes[i].start_time > time {
						play_info.current_render = i;
						break;
					}
				}
				play_info.render_queue.retain_mut(|inner| {
					if let Err(e) = inner.caculate(&time) {
						error!("{}", e);
					};
					(inner.start_time + inner.sustain_time > time) && (inner.start_time < time)
				});
				let mut judge_field_to_delete = vec!();
				for (id, (field, _)) in &mut play_info.judge_fields {
					if field.start_time + field.sustain_time < time {
						judge_field_to_delete.push(id.clone());
						continue;
					}
					if let Err(e) = field.caculate(&time) {
						error!("{}", e);
					};
				};
				for id in judge_field_to_delete {
					play_info.judge_fields.remove(&id);
				}
			}
			if let PlayMode::Auto = play_info.play_mode {
				self.judge(JudgeEvent::default())?;
			}
		}

		if let (Some(editor), Some((chart, _))) = (&mut self.chart_editor, &self.current_chart) {
			let is_keep = |select: &Select| -> bool {
				match select {
					Select::ClickEffect(id) => chart.click_effects.get(id).is_some(),
					Select::Note(id) => chart.notes.get(id).is_some(),
					Select::Shape(id) => chart.shapes.get(id).is_some(),
					Select::JudgeField(id) => chart.judge_fields.get(id).is_some(),
					Select::Script(_) => false,
				}
			};
			if let Some(t) = &editor.now_select {
				if !is_keep(t) {
					editor.now_select = None
				}
			}
			editor.multi_select.retain(|inner| is_keep(inner));
		}
		
		Ok(())
	}

	/// clear playing
	pub fn clear_play(&mut self) {
		self.play_info = None;
	}

	pub fn play_with_time(&mut self, play_mode: PlayMode, time: Duration) -> Result<(), Error> {
		self.play(play_mode)?;
		self.pause()?;
		self.set(time)?;
		self.resume()
	}

	/// start play current chart with given option, from start.
	pub fn play(&mut self, play_mode: PlayMode) -> Result<(), Error> {
		debug!("start playing..");
		let (chart, info) = if let Some((chart, info)) = &mut self.current_chart {
			let out = chart.clone();
			(out, info)
		}else {
			return Err(PlayError::NoChartLoaded.into());
		};
		let mut shapes = vec!();
		for (_, shape) in chart.shapes.into_iter() {
			shapes.push(shape);
		}
		shapes.sort_by(|a, b| a.start_time.cmp(&b.start_time));
		let mut judge_fields = HashMap::new();
		for (id, judge_field) in chart.judge_fields.into_iter() {
			judge_fields.insert(id.clone(), (judge_field, JudgeInfo {
				current_judge: 0,
				judge_tracks: vec!()
			}));
		}
		let mut notes: HashMap<String, Vec<Note>> = HashMap::new();
		for (_, note) in chart.notes.into_iter() {
			if let Some(t)  = notes.get_mut(&note.judge_field_id) {
				t.push(note);
			}else {
				notes.insert(note.judge_field_id.clone(), vec!(note));
			};
		}
		let mut total_notes = 0;
		for (_, vec) in &mut notes {
			vec.sort_by(|a, b| a.judge_time.cmp(&b.judge_time));
			total_notes = total_notes + vec.len();
		}
		let audio_manager = match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
			Ok(t) => t,
			Err(e) => return Err(PlayError::ManagerCreateFail(e).into()),
		};
		let play_info = Some(PlayInfo {
			shapes,
			judge_fields,
			notes,
			judge_vec: vec!(),
			render_queue: vec!(),
			score: 0.,
			accuracy: 0.,
			combo: 0,
			max_combo: 0,
			play_mode,
			replay: Replay::default(),
			audio_manager,
			total_notes,
			judged_notes: 0,
			click_effects: chart.click_effects.clone(),
			is_finished: false,
			sustain_time: info.sustain_time,
			current_render: 0,
			offcet: info.offcet,
			is_track_played: false,
			track_path: format!("{}/song.mp3",info.path.display()).into()
		});
		self.play_info = play_info;
		self.timer = Timer::default();
		self.timer.start()?;
		Ok(())
	}

	/// pause playing
	pub fn pause(&mut self) -> Result<(), Error> {
		if let Some(play_info) = &mut self.play_info {
			play_info.is_track_played = false;
			if let Err(e) = play_info.audio_manager.pause(Tween {
				duration: std::time::Duration::from_secs(1),
				..Default::default()
			}) {
				return Err(PlayError::from(e).into());
			}
		}else {
			return Err(PlayError::HaventStart.into())
		};
		self.timer.pause()
	}

	/// resume playing
	pub fn resume(&mut self) -> Result<(), Error> {
		if let Some(play_info) = &mut self.play_info {
			if let Err(e) = play_info.audio_manager.resume(Tween {
				duration: std::time::Duration::from_secs(1),
				..Default::default()
			}) {
				return Err(PlayError::from(e).into());
			}
		}else {
			return Err(PlayError::HaventStart.into())
		};
		self.timer.start()
	}

	/// get current time
	pub fn current(&mut self) -> Result<Duration, Error> {
		Ok(self.timer.read()? - Duration::seconds(3))
	}

	/// single select an element
	pub fn select(&mut self, select: Select) -> Result<(), Error> {
		if let Some(t) = &mut self.chart_editor {
			t.now_select = Some(select);
			return Ok(())
		}else {
			return Err(ChartEditError::NotInEditMode.into());
		}
	}

	/// multi select an element
	pub fn multi_select(&mut self, select: Vec<Select>) -> Result<(), Error> {
		if let Some(t) = &mut self.chart_editor {
			t.multi_select = select;
			return Ok(())
		}else {
			return Err(ChartEditError::NotInEditMode.into());
		}
	}


	/// get current select
	pub fn current_select(&mut self) -> Result<Option<Select>, Error> {
		if let Some(t) = &mut self.chart_editor {
			Ok(t.now_select.clone())
		}else {
			return Err(ChartEditError::NotInEditMode.into());
		}
	}

	/// get current selects
	pub fn current_selects(&mut self) -> Result<Vec<Select>, Error> {
		if let Some(t) = &mut self.chart_editor {
			let mut back = t.multi_select.clone();
			if let Some(inner) = &t.now_select {
				back.insert(0, inner.clone());
			}
			Ok(back)
		}else {
			return Err(ChartEditError::NotInEditMode.into());
		}
	}

	/// clear select
	pub fn clear_select(&mut self) -> Result<(), Error> {
		if let Some(t) = &mut self.chart_editor {
			t.now_select = None;
		}else {
			return Err(ChartEditError::NotInEditMode.into());
		}
		Ok(())
	}

	/// clear selects
	pub fn clear_selects(&mut self) -> Result<(), Error> {
		if let Some(t) = &mut self.chart_editor {
			t.now_select = None;
			t.multi_select = vec!();
		}else {
			return Err(ChartEditError::NotInEditMode.into());
		}
		Ok(())
	}

	/// set chart play time
	pub fn set(&mut self, input: Duration) -> Result<(), Error> {
		self.timer.set(input + Duration::seconds(3))?;
		if let Some(t) = &mut self.play_info {
			t.current_render = 0;
		}
		Ok(())
	}

	/// edit current chart
	pub fn edit(&mut self) -> Result<(), Error> {
		if self.current_chart.is_none() {
			return Err(PlayError::NoChartLoaded.into())
		}
		self.chart_editor = Some(Default::default());
		self.play(PlayMode::Auto)?;
		self.timer = Timer::default();
		self.timer.start()?;
		self.pause()?;
		Ok(())
	}

	/// judge something, if this was called while PlayMode is Auto or Replay, it will do nothing
	pub fn judge(&mut self, event: JudgeEvent) -> Result<(), Error> {
		debug!("judging notes...");
		if let Some(play_info) = &mut self.play_info {
			play_info.judge(event, &self.timer)?;
		}
		Ok(())
	}

	/// create new chart with giving info
	pub fn create_new_chart(&mut self, song_name: String, producer: String, charter: String, artist: String, track_path: PathBuf, image_path: PathBuf) -> Result<(), Error> {
		if song_name.is_empty() | producer.is_empty() | charter.is_empty() | artist.is_empty() | (track_path == PathBuf::new()) | (image_path == PathBuf::new()) {
			return Err(ChartEditError::MissingInfo.into());
		}

		let path = format!("{}/shapoist_assets/chart/{}", self.assets_path.display(), song_name);
		create_dir(&path)?;
		let length = match StaticSoundData::from_file(&track_path, StaticSoundSettings::default()) {
			Ok(t) => t.duration().as_secs_f32(),
			Err(e) => return Err(ChartError::MusicSourceCantRead(e).into()),
		};

		let mut chart_info = ChartInfo {
			song_name,
			producer,
			charter,
			artist,
			path: path.clone().into(),
			sustain_time: Duration::seconds_f32(length),
			..Default::default()
		};
		create_file(format!("{}/config.toml", path))?;
		write_file(format!("{}/config.toml", path), to_toml(&chart_info)?.as_bytes())?;
		create_file(format!("{}/chart.sc", path))?;
		write_file(format!("{}/chart.sc", path), to_toml(&Chart::default())?.as_bytes())?;
		copy_file(image_path, format!("{}/back.png", path).into())?;
		copy_file(track_path, format!("{}/song.mp3", path).into())?;
		self.read_chart(&mut chart_info)
	}

	pub fn save_current_chart(&mut self) -> Result<(), Error> {
		if let Some((chart, info)) = &self.current_chart {
			let path = format!("{}",info.path.display());
			write_file(format!("{}/chart.sc", path), to_toml(&chart)?.as_bytes())?;
			write_file(format!("{}/config.toml", path), to_toml(&info)?.as_bytes())?;
			Ok(())
		}else {
			return Err(PlayError::NoChartLoaded.into())
		}
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
		let settings = parse_toml(&match read_file_to_string(settings_path.clone()) {
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
			PathBuf::from(format!("{}/chart.sc",self.path.display())).exists() &&
			PathBuf::from(format!("{}/config.toml",self.path.display())).exists());
		if is_file_missing {
			return Err(ChartError::FileMissing.into());
		}

		let chart = match read_file_to_string(format!("{}/chart.sc", self.path.display())) {
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

	/// get all beats according to BPM
	pub fn total_beats(&self) -> f32 {
		if self.bpm.linkers.is_empty() {
			let bps = self.bpm.start_bpm / 60.0;
			self.sustain_time.as_seconds_f32() * bps
		}else {
			let mut beats = 0.0;
			let mut last_bps = self.bpm.start_bpm / 60.0;
			for linker in &self.bpm.linkers {
				let linker_bps = linker.bpm / 60.0;
				beats = beats + match linker.linker {
					BpmLinkerType::Bezier(_, _) => todo!(),
					BpmLinkerType::Linear => linker.time * (last_bps + linker_bps) / 2.0,
					BpmLinkerType::Mutation => linker.time * last_bps,
					BpmLinkerType::Power(n) => linker.time * (n * last_bps + linker_bps) / (n + 1.0),
				}.as_seconds_f32();
				last_bps = linker.bpm / 60.0;
			}
			beats
		}
	}

	/// get how many beats do given duration contains
	pub fn beats(&self, start_time: &Duration, end_time: &Duration) -> f32 {
		let delta = *end_time - *start_time;
		if self.bpm.linkers.is_empty() {
			let bps = self.bpm.start_bpm / 60.0;
			delta.as_seconds_f32() * bps
		}else {
			todo!()
		}
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

impl PlayInfo {
	fn judge(&mut self, event: JudgeEvent, timer: &Timer) -> Result<(), Error> {
		let time = timer.read()? - Duration::seconds(3);
		if time < Duration::ZERO {
			return Ok(())
		}
		let mut judges = vec!();
		match self.play_mode {
			PlayMode::Normal => {
				for (id, (field, judge_track)) in &mut self.judge_fields {
					if let Some(notes) = self.notes.get_mut(id) {
						for click in &event.clicks {
							judge_track.judge_tracks.retain_mut(|inner| {
								if inner.linked_click != click.id {
									true
								}else {
									let note = &notes[inner.note_id];
									let mut judge: Option<Judge> = None;
									match note.judge_type {
										JudgeType::Flick => {
											if (click.position - inner.last_position).len() > 5.0 {
												judge = Some(Judge::Immaculate(1.0));
											}else {
												judge = Some(Judge::Miss)
											}
										},
										JudgeType::TapAndFlick => {
											if (click.position - inner.last_position).len() > 5.0 {
												judge = Some(Judge::Immaculate(1.0));
											}else {
												judge = Some(Judge::Miss)
											}
										},
										JudgeType::Hold(sustain) => {
											let delta = (inner.start_time - note.judge_time).abs();
											let percent = ((time - note.judge_time) / sustain) as f32;
											let judge_check = || -> Judge {
												if delta < Duration::milliseconds(50) {
													Judge::Immaculate((delta / Duration::milliseconds(50)) as f32)
												}else if delta < Duration::milliseconds(90) {
													Judge::Extra
												}else if delta < Duration::milliseconds(130) {
													Judge::Fade
												}else {
													Judge::Miss
												}
											};
											if percent > 1.0 {
												let mut judge_inner = judge_check();
												if let Judge::Immaculate(inner) = &mut judge_inner {
													*inner = *inner * 0.8
												}
												judge = Some(judge_inner);
											}else if percent > 0.0 {
												if let ClickState::Released = click.state {
													if percent < 0.8 {
														judge = Some(Judge::Miss)
													}else {
														let mut judge_inner = judge_check();
														if let Judge::Immaculate(inner) = &mut judge_inner {
															*inner = *inner * percent
														}
														judge = Some(judge_inner);
													}
												}
											}
										},
										JudgeType::AngledFilck(angle) => {
											let delta = click.position - inner.last_position;
											let radio = (delta.angle() - angle).abs() / (std::f32::consts::PI / 12.0);
											if delta.len() > 5.0 && radio < 1.0 {
												judge = Some(Judge::Immaculate(1.0 * radio));
											}else {
												judge = Some(Judge::Miss);
											}
										}
										JudgeType::AngledTapFilck(angle) => {
											let delta = click.position - inner.last_position;
											let radio = (delta.angle() - angle).abs() / (std::f32::consts::PI / 12.0);
											if delta.len() > 5.0 && radio < 1.0 {
												judge = Some(Judge::Immaculate(1.0 * radio));
											}else {
												judge = Some(Judge::Miss);
											}
										}
										JudgeType::Chain(_) => todo!(),
										JudgeType::TapChain(_) => todo!(),
										_ => {},
									}
									if let Some(judge_inner) = &judge {
										if let Some(shapes) = self.click_effects.get(&note.click_effect_id) {
											let mut shapes = shapes.get_shape(&time, &judge_inner);
											self.render_queue.append(&mut shapes);
										}
										judges.push(judge_inner.clone());
									}
									judge.is_none()
								}
							});
						}
						for note_id in judge_track.current_judge..notes.len() {
							let delta = time - notes[note_id].judge_time;
							if delta < Duration::milliseconds(-150) {
								break;
							}
							if delta > Duration::milliseconds(150) {
								judges.push(Judge::Miss);
								if let Some(shapes) = self.click_effects.get(&notes[note_id].click_effect_id) {
									let mut shapes = shapes.get_shape(&time, &Judge::Miss);
									self.render_queue.append(&mut shapes);
								}
								judge_track.current_judge = judge_track.current_judge + 1;
								continue;
							}
							for click in &event.clicks {
								let area = Area::new(field.inner.min, field.inner.max).transform(&Style {
									position: field.inner.position,
									size: field.inner.scale,
									rotate: field.inner.rotate,
									transform_origin: field.inner.transform_origin,
									..Default::default()
								});
								if area.is_point_inside(&click.position) { 
									let track = JudgeTrack {
										note_id,
										linked_click: click.id,
										start_time: time,
										last_position: click.position,
									};
									match (&click.state, &notes[note_id].judge_type) {
										(ClickState::Pressed, JudgeType::Hold(_)) | 
										(ClickState::Pressed, JudgeType::TapAndFlick) | 
										(ClickState::Pressed, JudgeType::TapChain(_)) | 
										(ClickState::Pressed, JudgeType::AngledTapFilck(_)) => {
											judge_track.judge_tracks.push(track);
											judge_track.current_judge = judge_track.current_judge + 1;
										},
										(_, JudgeType::Flick) | (_, JudgeType::Chain(_)) | (_, JudgeType::AngledFilck(_)) => {
											judge_track.judge_tracks.push(track);
											judge_track.current_judge = judge_track.current_judge + 1;
										},
										(ClickState::Pressed, JudgeType::Tap) => {
											let judge;
											let delta = delta.abs();
											if delta < Duration::milliseconds(50) {
												judge = Judge::Immaculate((delta / Duration::milliseconds(50)) as f32);
											}else if delta < Duration::milliseconds(90) {
												judge = Judge::Extra
											}else if delta < Duration::milliseconds(130) {
												judge = Judge::Fade
											}else {
												judge = Judge::Miss;
											}

											judge_track.current_judge = judge_track.current_judge + 1;
											if let Some(shapes) = self.click_effects.get(&notes[note_id].click_effect_id) {
												let mut shapes = shapes.get_shape(&time, &judge);
												self.render_queue.append(&mut shapes);
											}
											judges.push(judge);
										},
										(_, JudgeType::Slide) => {
											let judge = Judge::Immaculate(1.0);

											judge_track.current_judge = judge_track.current_judge + 1;
											if let Some(shapes) = self.click_effects.get(&notes[note_id].click_effect_id) {
												let mut shapes = shapes.get_shape(&time, &judge);
												self.render_queue.append(&mut shapes);
											}
											judges.push(judge);
										},
										_ => {},
									}
								}
							}
						}
					}
				}
			},
			PlayMode::Auto => {
				for (id, (_, judge_track)) in &mut self.judge_fields {
					if let Some(notes) = self.notes.get_mut(id) {
						for note_id in judge_track.current_judge..notes.len() {
							if notes[note_id].judge_time > time {
								let judge = Judge::Immaculate(1.0);
								judge_track.current_judge = judge_track.current_judge + 1;
								if let Some(shapes) = self.click_effects.get(&notes[note_id].click_effect_id) {
									let mut shapes = shapes.get_shape(&time, &judge);
									self.render_queue.append(&mut shapes);
								}
								judges.push(judge);
							}
						}
					}
				}
			},
			PlayMode::Replay(_) => todo!()
		}
		for judge in judges {
			self.caculate(judge);
		}
		Ok(())
	}

	#[inline]
	fn caculate(&mut self, judge: Judge) {
		self.judged_notes = self.judged_notes + 1;
		match judge {
			Judge::Fade | Judge::Miss => self.combo = 0,
			_ => self.combo = self.combo + 1,
		}
		if self.combo > self.max_combo {
			self.max_combo = self.combo;
		}

		let acc = match judge {
			Judge::Immaculate(inner) => 0.8 + inner * 0.2,
			Judge::Extra => 0.7,
			Judge::Normal => 0.3,
			Judge::Fade => 0.1,
			Judge::Miss => 0.0
		};
		self.accuracy = (self.accuracy * (self.judged_notes - 1) as f32 + acc) / self.judged_notes as f32;
		self.score = self.accuracy * self.judged_notes as f32 / self.total_notes as f32 * 2.0 * 1e7 * 0.9 + self.max_combo as f32 / self.total_notes as f32 * 0.1 * 2.0 * 1e7;
		self.judge_vec.push(judge);
	}
}

impl ClickEffect {
	fn get_shape(&self, time: &Duration, judge: &Judge) -> Vec<Shape> {
		let mut shapes = match judge {
			Judge::Immaculate(_) => {
				self.immaculate_effect.clone()
			}
			Judge::Extra => {
				self.extra_effect.clone()
			}
			Judge::Normal => {
				self.normal_effect.clone()
			}
			Judge::Fade => {
				self.fade_effect.clone()
			}
			Judge::Miss => {
				self.miss_effect.clone()
			}
		};
		for shape in &mut shapes {
			shape.start_time = shape.start_time + *time;
			for (_, animation) in &mut shape.animation {
				animation.start_time = animation.start_time + *time;
			}
		}
		shapes
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
			charts.push(ChartInfo::process(&chart)?);
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
			assets_path: PathBuf::from("./"),
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