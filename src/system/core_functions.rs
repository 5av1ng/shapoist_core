use crate::CLICK_SOUND;
use crate::DELAY_ADJUSTMENT;
use nablo_shape::prelude::Vec2;
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
// #[cfg(not(target_arch = "wasm32"))]
use kira::sound::static_sound::StaticSoundData;
// #[cfg(not(target_arch = "wasm32"))]
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
		info!("checking initlizaition infomation from path: {}", assets_path);
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
			Err(e) => {
				Err(e)
			}
		}
	} 

	/// create minimal core, used in wasm.
	pub fn minimal() -> Self {
		debug!("creating new minimal ShapoistCore struct..");
		// web user should be able to get their setting by login.

		Self::default()
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
		if !self.command_history.is_empty() && command != self.command_history[self.command_history.len() - 1] {
			self.command_history.push(command)
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
			play_info.frame(&mut self.timer, self.settings.offset, true, if let Some(editor) = &self.chart_editor {
				editor.show_click_effect
			}else {
				true
			}, &mut self.current_sound, &mut self.is_in_delay_adjustment, &self.settings)?;
		}

		if let (Some(editor), Some((chart, _))) = (&mut self.chart_editor, &self.current_chart) {
			let is_keep = |select: &Select| -> bool {
				match select {
					Select::ClickEffect(id) => chart.click_effects.contains_key(id),
					Select::Note(id) => chart.notes.contains_key(id),
					Select::Shape(id) => chart.shapes.contains_key(id),
					Select::JudgeField(id) => chart.judge_fields.contains_key(id),
					Select::Script(_) => false,
				}
			};
			if let Some(t) = &editor.now_select {
				if !is_keep(t) {
					editor.now_select = None
				}
			}
			editor.multi_select.retain(is_keep);
		}
		
		Ok(())
	}

	pub fn update_render_queue(&mut self, will_play_music: bool) -> Result<(), Error> {
		if let Some(play_info) = &mut self.play_info {
			play_info.frame(&mut self.timer, self.settings.offset, will_play_music, if let Some(editor) = &self.chart_editor {
				editor.show_click_effect
			}else {
				true
			}, &mut self.current_sound, &mut self.is_in_delay_adjustment, &self.settings)
		}else {
			Err(PlayError::HaventStart.into())
		}
	}

	/// clear playing
	pub fn clear_play(&mut self) {
		self.timer.pause();
		self.current_sound = None;
		self.play_info = None;
		self.is_in_delay_adjustment = false;
	}

	/// clear editing
	pub fn clear_edit(&mut self) {
		self.chart_editor = None;
	}

	/// as name says
	pub fn play_with_time(&mut self, play_mode: PlayMode, time: Duration) -> Result<(), Error> {
		self.play(play_mode)?;
		debug!("setting timer...");
		self.pause()?;
		self.timer.set_to(time + Duration::seconds(3));
		if let Some(t) = &mut self.play_info {
			t.current_render = 0;
		}
		debug!("setted timer...");
		self.resume()
	}

	/// start play current chart with given option, from start.
	pub fn play(&mut self, play_mode: PlayMode) -> Result<(), Error> {
		debug!("start playing..");
		let (mut chart, info) = if let Some((chart, info)) = &mut self.current_chart {
			let out = chart.clone();
			(out, info)
		}else {
			return Err(PlayError::NoChartLoaded.into());
		};
		debug!("moving sources");
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
		for vec in notes.values_mut() {
			vec.sort_by(|a, b| a.judge_time.cmp(&b.judge_time));
			total_notes += vec.len();
		}
		debug!("setting audio...");
		let audio_manager = match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
			Ok(t) => t,
			Err(e) => return Err(PlayError::ManagerCreateFail(e).into()),
		};
		let click_sound = match StaticSoundData::from_cursor(std::io::Cursor::new(CLICK_SOUND), StaticSoundSettings::default().volume(self.settings.click_sound_volume as f64)) {
			Ok(t) => t,
			Err(e) => {
				return Err(ChartError::MusicSourceCantReadString(e.to_string()).into());
			}
		};
		chart.events.sort_by(|a, b| a.time.cmp(&b.time));
		let events = chart.events;
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
			offset: info.offset,
			is_track_played: false,
			track_path: format!("{}/song.mp3",info.path.display()).into(),
			#[cfg(target_arch = "wasm32")]
			sound_data: vec!(),
			judged_note_id: vec!(),
			click_sound,
			click_sound_handle: None,
			events,
			current_event: 0,
			current_shader: None,
			shaders: chart.shaders.clone(),
		});
		self.play_info = play_info;
		debug!("setting timer...");
		self.timer = Timer::default();
		self.timer.start();
		debug!("started play");
		Ok(())
	}

	/// if chart have changed, you should call this
	pub fn refresh_play_info(&mut self) -> Result<(), Error> {
		let chart = if let Some((chart, _)) = &mut self.current_chart {
			chart.clone()
		}else {
			return Err(PlayError::NoChartLoaded.into());
		};
		if let Some(play_info) = &mut self.play_info {
			debug!("moving sources");
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
			for vec in notes.values_mut() {
				vec.sort_by(|a, b| a.judge_time.cmp(&b.judge_time));
				total_notes += vec.len();
			}
			play_info.shapes = shapes;
			play_info.judge_fields = judge_fields;
			play_info.notes = notes;
			play_info.total_notes = total_notes;
			play_info.current_render = 0;
			play_info.render_queue.clear();
			Ok(())
		}else {
			Err(PlayError::HaventStart.into())
		}
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
		self.timer.pause();
		Ok(())
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
		self.timer.start();
		Ok(())
	}

	/// get current time
	pub fn current(&mut self) -> Result<Duration, Error> {
		Ok(self.timer.read() - Duration::seconds(3))
	}

	/// single select an element
	pub fn select(&mut self, select: Select) -> Result<(), Error> {
		if let Some(t) = &mut self.chart_editor {
			t.now_select = Some(select);
			Ok(())
		}else {
			Err(ChartEditError::NotInEditMode.into())
		}
	}

	/// multi select an element
	pub fn multi_select(&mut self, select: Vec<Select>) -> Result<(), Error> {
		if let Some(t) = &mut self.chart_editor {
			t.multi_select = select;
			Ok(())
		}else {
			Err(ChartEditError::NotInEditMode.into())
		}
	}


	/// get current select
	pub fn current_select(&mut self) -> Result<Option<Select>, Error> {
		if let Some(t) = &mut self.chart_editor {
			Ok(t.now_select.clone())
		}else {
			Err(ChartEditError::NotInEditMode.into())
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
			Err(ChartEditError::NotInEditMode.into())
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
		self.timer.set(input + Duration::seconds(3));
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
		self.timer.start();
		self.pause()?;
		Ok(())
	}

	/// judge something, if this was called while PlayMode is Auto or Replay, it will do nothing
	pub fn judge(&mut self, event: JudgeEvent) -> Result<(), Error> {
		debug!("judging notes...");
		if let Some(play_info) = &mut self.play_info {
			play_info.judge(event, &self.timer, if let Some(editor) = &self.chart_editor {
				editor.show_click_effect
			}else {
				true
			})?;
		}
		Ok(())
	}

	/// as name says
	pub fn start_delay_adjustment(&mut self) -> Result<(), Error> {
		info!("starting delay adjustment");
		self.adjustment.clear();
		self.is_in_delay_adjustment = true;
		self.current_chart = Some((Default::default(), ChartInfo {
			sustain_time: Duration::seconds(16),
			..Default::default()
		}));
		self.play(PlayMode::Auto)?;
		self.current_sound = match StaticSoundData::from_media_source(std::io::Cursor::new(DELAY_ADJUSTMENT), Default::default()) {
			Ok(t) => Some(t),
			Err(e) => {
				return Err(ChartError::MusicSourceCantReadString(e.to_string()).into());
			}
		};
		Ok(())
	}

	/// judge something, but in delay adjustment, returns true if adjustment finished
	pub fn delay_adjustment_judge(&mut self, event: JudgeEvent) -> Result<bool, Error> {
		debug!("judging notes in delay adjustment...");
		let current = self.timer.read() - Duration::seconds(3);
		if current > Duration::seconds(16) {
			self.is_in_delay_adjustment = false;
			self.clear_play();
			return Ok(true);
		}
		for click in event.clicks {
			if let ClickState::Pressed = click.state {
				for i in 0..16 {
					if (2 * i - 1) * Duration::milliseconds(500) <= current && current < (2 * i + 1) * Duration::milliseconds(500) {
						self.adjustment.push(current - i * Duration::milliseconds(1000));
						return Ok(false)
					}
				}
			}
		}
		Ok(false)
	}

	/// returns offset value and variance.
	pub fn current_delay(&self) -> (Duration, f32) {
		if self.adjustment.is_empty() {
			return (Duration::ZERO, 0.0)
		}
		let average: Duration = self.adjustment.iter().sum::<Duration>() / self.adjustment.len() as f32;
		let variance = self.adjustment.iter().map(|value| {
			(value.as_seconds_f32() - average.as_seconds_f32()).powf(2.0)
		}).sum::<f32>() / self.adjustment.len() as f32;
		(average, variance)
	}

	/// create new chart with giving info
	#[cfg(not(target_arch = "wasm32"))]
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

	#[cfg(target_arch = "wasm32")]
	/// create new chart with giving info
	pub fn create_new_chart(&mut self, _: String, _: String, _: String, _: String, _: PathBuf, _: PathBuf) -> Result<(), Error> {
		Err(Error::PlatformUnsupport(String::from("wasm32")))
	}

	/// as name says
	pub fn save_current_chart(&mut self) -> Result<(), Error> {
		if let Some((chart, info)) = &self.current_chart {
			let path = format!("{}",info.path.display());
			write_file(format!("{}/chart.sc", path), to_toml(&chart)?.as_bytes())?;
			write_file(format!("{}/config.toml", path), to_toml(&info)?.as_bytes())?;
			Ok(())
		}else {
			Err(PlayError::NoChartLoaded.into())
		}
	}

	/// as name says
	pub fn delete_current_chart(&mut self) -> Result<(), Error> {
		let info = if let Some((_, info)) = self.current_chart.as_ref() {
			info.clone()
		}else {
			return Err(PlayError::NoChartLoaded.into())
		};
		self.current_chart = None;
		self.delete_chart(&info)
	}

	/// as name says
	pub fn delete_chart(&mut self, chart_info: &ChartInfo) -> Result<(), Error> {
		self.chart_list.retain(|inner| inner != chart_info);
		remove_path(&chart_info.path)
	}

	/// copy current selected values, filter returns ture means need to copy
	pub fn copy_select(&mut self, current: Duration, filter: impl Fn(&Select) -> bool) -> Result<(), Error> {
		let current_selects = self.current_selects()?;
		if let Some(editor) = &mut self.chart_editor {
			editor.clone_buffer.clear();
			if let Some((chart, _)) = &mut self.current_chart {
				for select in current_selects {
					if filter(&select) {
						match select {
							Select::Note(id) => {
								if let Some(inner) = chart.notes.get(&id) {
									let mut inner = inner.clone();
									inner.judge_time -= current;
									editor.clone_buffer.push(SelectUnchange::Note(id, inner))
								}
							},
							Select::Shape(id) => {
								if let Some(inner) = chart.shapes.get(&id) {
									let mut inner = inner.clone();
									inner.start_time -= current;
									for animation in inner.animation.values_mut() {
										animation.start_time -= current;
									}
									editor.clone_buffer.push(SelectUnchange::Shape(id, inner))
								}
							},
							Select::JudgeField(id) => {
								if let Some(inner) = chart.judge_fields.get(&id) {
									let mut inner = inner.clone();
									inner.start_time -= current;
									for animation in inner.animation.values_mut() {
										animation.start_time -= current;
									}
									editor.clone_buffer.push(SelectUnchange::JudgeField(id, inner))
								}
							},
							Select::ClickEffect(_) => {},
							Select::Script(_) => {},
						}
					}
				}
				Ok(())
			}else {
				Err(PlayError::NoChartLoaded.into())
			}
		}else {
			Err(ChartEditError::NotInEditMode.into())
		}
	}

	/// paste copied Error ouccurs when not copied
	pub fn paste_select(&mut self, current: Duration) -> Result<(), Error> {
		if let Some(editor) = &mut self.chart_editor {
			if let Some((chart, _)) = &mut self.current_chart {
				let inner = editor.clone_buffer.clone();
				for inner in inner {
					match inner {
						SelectUnchange::Shape(id, mut t) => {
							let mut index = 2;
							t.start_time += current;
							for animation in t.animation.values_mut() {
								animation.start_time += current;
							}
							loop {
								if let std::collections::hash_map::Entry::Vacant(e) = chart.shapes.entry(format!("{} #{}", id, index)) {
									t.id = format!("{} #{}", id, index);
									e.insert(t);
									break;
								}
								index += 1;
							}
							
						},
						SelectUnchange::JudgeField(id, mut t) => {
							let mut index = 2;
							t.start_time += current;
							for animation in t.animation.values_mut() {
								animation.start_time += current;
							}
							loop {
								if let std::collections::hash_map::Entry::Vacant(e) = chart.judge_fields.entry(format!("{} #{}", id, index)) {
									e.insert(t);
									break;
								}
								index += 1;
							}
						},
						SelectUnchange::Note(id, mut t) => {
							let mut index = 2;
							t.judge_time += current;
							loop {
								if let std::collections::hash_map::Entry::Vacant(e) = chart.notes.entry(format!("{} #{}", id, index)) {
									t.note_id = format!("{} #{}", id, index);
									e.insert(t);
									break;
								}
								index += 1;
							}
						},
						SelectUnchange::ClickEffect(_, _) => {},
						SelectUnchange::Script(_, _) => {},
					}
				}
				Ok(())
			}else {
				Err(PlayError::NoChartLoaded.into())
			}
		}else {
			Err(ChartEditError::NotInEditMode.into())
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
	let now = time::OffsetDateTime::now_utc();
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
		Ok(ChartInfo {
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
			Err(e) => {
				return Err(e)
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
				beats += match linker.linker {
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
			Ok(ScriptInfo {
				condition: Condition::Broken,
				path: PathBuf::from(path),
				..Default::default()
			})
		}else {
			Ok(ScriptInfo {
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
	/// call this function to render a single frame
	#[allow(clippy::too_many_arguments)]
	pub fn frame(&mut self, timer: &mut Timer, offset: Duration, will_play_music: bool, show_click_effect: bool, sound: &mut Option<StaticSoundData>, is_in_delay_adjustment: &mut bool, settings: &Settings) -> Result<(), Error> {
		let time = timer.read();
		if self.is_finished {
			debug!("play finished");
			*is_in_delay_adjustment = false;
			timer.pause();
			if let Err(e) = self.audio_manager.pause(Tween {
				duration: std::time::Duration::from_secs(1),
				..Default::default()
			}) {
				return Err(PlayError::from(e).into());
			}
		}

		if !self.is_finished {
			let play_time = if *is_in_delay_adjustment {
				time + self.offset - Duration::seconds(3) 
			}else { 
				time + self.offset + offset - Duration::seconds(3) 
			};
			if !self.is_track_played && play_time > Duration::ZERO && will_play_music {
				info!("music playing...");
				let sound_setting = StaticSoundSettings::new().playback_region(RangeFrom { start: play_time.as_seconds_f64() }).volume(settings.music_volume as f64);
				let static_sound = if let Some(sound) = sound {
					sound.clone().with_settings(sound_setting)
				}else {
					cfg_if::cfg_if! {
						if #[cfg(target_arch = "wasm32")] {
							let out = match StaticSoundData::from_media_source(std::io::Cursor::new(self.sound_data.clone()), sound_setting) {
								Ok(t) => t,
								Err(e) => return Err(ChartError::from(e).into()),
							};
							*sound = Some(out.clone());
							out
						}else {
							let out = match StaticSoundData::from_file(&self.track_path, sound_setting) {
								Ok(t) => t,
								Err(e) => return Err(ChartError::from(e).into()),
							};
							*sound = Some(out.clone());
							out
						}
					}
				};
				
				if let Err(e) = self.audio_manager.play(static_sound) {
					return Err(PlayError::from(e).into());
				};

				self.is_track_played = true;
				info!("music played");
			}
		}

		if !self.is_finished && time > Duration::seconds(3) {
			let time = time - Duration::seconds(3);

			if time > self.sustain_time {
				self.is_finished = true;
				self.shapes.clear();
				self.notes.clear();
				self.judge_fields.clear();
				self.click_effects.clear();
			};
			for i in self.current_render..self.shapes.len() {
				if self.shapes[i].start_time <= time {
					self.render_queue.push(self.shapes[i].clone());
					self.current_render = i + 1;
				}else {
					// self.current_render = i;
					break;
				}
			}
			self.render_queue.retain_mut(|inner| {
				if let Err(e) = inner.caculate(&time) {
					error!("{}", e);
				};
				if let Some(t) = &inner.linked_note_id {
					let mut is_containing_linked_note = false;
					for id in t {
						if self.judged_note_id.contains(id) {
							is_containing_linked_note = true
						}
					}
					((inner.start_time + inner.sustain_time >= time) && (inner.start_time <= time)) && !is_containing_linked_note
				}else {
					(inner.start_time + inner.sustain_time >= time) && (inner.start_time <= time)
				}
			});
			let mut judge_field_to_delete = vec!();
			for (id, (field, _)) in &mut self.judge_fields {
				if field.start_time + field.sustain_time < time {
					judge_field_to_delete.push(id.clone());
					continue;
				}
				if let Err(e) = field.caculate(&time) {
					error!("{}", e);
				};
			};
			for id in judge_field_to_delete {
				self.judge_fields.remove(&id);
			}

			loop {
				if self.events[self.current_event].time > time {
					break;
				}
				if let ChartEventInner::ChangeShader(id) = &self.events[self.current_event].inner {
					if let Some(id) = id {
						self.current_shader = self.shaders.get(id).cloned();
					}else {
						self.current_shader = None;
					}
				}
				self.current_event += 1;
			}
		}
		if let PlayMode::Auto = self.play_mode {
			self.judge(JudgeEvent::default(), timer, show_click_effect)?;
		}
		Ok(())
	}

	fn judge(&mut self, event: JudgeEvent, timer: &Timer, show_click_effect: bool) -> Result<(), Error> {
		let time = timer.read() - Duration::seconds(3);
		if time < Duration::ZERO {
			return Ok(())
		}
		let mut judges = vec!();

		let click_effect_position = |note: &Note, render_queue: &Vec<Shape>| -> Vec2 {
			if let Some(id) = &note.linked_shape {
				let mut output: Option<Vec2> = None;
				let mut i = 1.0;
				for id in id {
					for shape in render_queue {
						if &shape.id == id {
							if let Some(inner) = &mut output {
								*inner = (*inner * i + shape.shape.get_area().center()) / (i + 1.0);
								i += 1.0;
							}else {
								output = Some(shape.shape.get_area().center());
							}
						}
					}
				}
				if let Some(inner) = output {
					inner
				}else {
					note.click_effect_position
				}
			}else {
				note.click_effect_position
			}
		};

		match self.play_mode {
			PlayMode::Normal => {
				for (id, (field, judge_track)) in &mut self.judge_fields {
					if let Some(notes) = self.notes.get_mut(id) {
						for click in &event.clicks {
							judge_track.judge_tracks.retain_mut(|inner| {
								let note = &notes[inner.note_id];
								let mut judge: Option<Judge> = None;
								match note.judge_type {
									JudgeType::Flick => {
										if inner.linked_click == click.id {
											if (click.position - inner.last_position).len() > 5.0 {
												judge = Some(Judge::Immaculate(1.0));
											}
											if (time - inner.start_time).abs() > Duration::milliseconds(120) {
												judge = Some(Judge::Miss);
											}
										}
										
									},
									JudgeType::TapAndFlick => {
										if inner.linked_click == click.id {
											if (click.position - inner.last_position).len() > 5.0 {
												judge = Some(Judge::Immaculate(1.0));
											}
											if (time - inner.start_time).abs() > Duration::milliseconds(120) {
												judge = Some(Judge::Miss);
											}
										}
									},
									JudgeType::Hold(sustain) => {
										let delta = (inner.start_time - note.judge_time).abs();
										let percent = ((time - note.judge_time) / sustain) as f32;
										let judge_check = || -> Judge {
											if delta < Duration::milliseconds(50) {
												Judge::Immaculate((delta / Duration::milliseconds(50)) as f32)
											}else if delta < Duration::milliseconds(70) {
												Judge::Extra
											}else if delta < Duration::milliseconds(120) {
												Judge::Normal
											}else if delta < Duration::milliseconds(150) {
												Judge::Fade
											}else {
												Judge::Miss
											}
										};
										if percent > 1.0 {
											let mut judge_inner = judge_check();
											if let Judge::Immaculate(inner) = &mut judge_inner {
												*inner *= 0.995
											}
											judge = Some(judge_inner);
										}else if percent > 0.0 {
											if let ClickState::Released = click.state {
												if percent < 0.8 {
													judge = Some(Judge::Miss)
												}else {
													let mut judge_inner = judge_check();
													if let Judge::Immaculate(inner) = &mut judge_inner {
														*inner *= percent
													}
													judge = Some(judge_inner);
												}
											}
										}
									},
									JudgeType::AngledFilck(angle) => {
										let delta = click.position - inner.last_position;
										let radio = (delta.angle() - angle).abs() / (std::f32::consts::PI / 12.0);
										if inner.linked_click == click.id {
											if delta.len() > 5.0 && radio < 1.0 {
												judge = Some(Judge::Immaculate(1.0 * radio));
											}
											if (time - inner.start_time).abs() > Duration::milliseconds(120) {
												judge = Some(Judge::Miss);
											}
										}
									}
									JudgeType::AngledTapFilck(angle) => {
										let delta = click.position - inner.last_position;
										let radio = (delta.angle() - angle).abs() / (std::f32::consts::PI / 12.0);
										if inner.linked_click == click.id {
											if delta.len() > 5.0 && radio < 1.0 {
												judge = Some(Judge::Immaculate(1.0 * radio));
											}
											if (time - inner.start_time).abs() > Duration::milliseconds(120) {
												judge = Some(Judge::Miss);
											}
										}
									}
									JudgeType::Chain(_) => todo!(),
									JudgeType::TapChain(_) => todo!(),
									_ => {},
								}
								if let Some(judge_inner) = &judge {
									if show_click_effect {
										let mut shapes = match self.click_effects.get(&note.click_effect_id) {
											Some(t) => t.clone(),
											None => Default::default()
										}.get_shape(&time, judge_inner, click_effect_position(note, &self.render_queue), &note.note_id);
										self.render_queue.append(&mut shapes);
									}
									judges.push(judge_inner.clone());
									self.judged_note_id.push(notes[inner.note_id].note_id.clone());
								}
								judge.is_none()
							});
						}
						for (id, note) in notes.iter_mut().enumerate() {
							let delta = time - note.judge_time;
							if delta < Duration::milliseconds(-150) {
								break;
							}
							if delta > Duration::milliseconds(150) {
								judges.push(Judge::Miss);
								self.judged_note_id.push(note.note_id.to_string());
								if show_click_effect {
									let mut shapes = match self.click_effects.get(&note.click_effect_id) {
										Some(t) => t.clone(),
										None => Default::default()
									}.get_shape(&time, &Judge::Miss, click_effect_position(note, &self.render_queue), &note.note_id);
									self.render_queue.append(&mut shapes);
								}
								judge_track.current_judge += 1;
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
										note_id: id,
										linked_click: click.id,
										start_time: time,
										last_position: click.position,
									};
									match (&click.state, &note.judge_type) {
										(ClickState::Pressed, JudgeType::Hold(_)) | 
										(ClickState::Pressed, JudgeType::TapAndFlick) | 
										(ClickState::Pressed, JudgeType::TapChain(_)) | 
										(ClickState::Pressed, JudgeType::AngledTapFilck(_)) => {
											judge_track.judge_tracks.push(track);
											judge_track.current_judge += 1;
										},
										(_, JudgeType::Flick) | (_, JudgeType::Chain(_)) | (_, JudgeType::AngledFilck(_)) => {
											judge_track.judge_tracks.push(track);
											judge_track.current_judge += 1;
										},
										(ClickState::Pressed, JudgeType::Tap) => {
											let judge;
											let delta = delta.abs();
											if delta < Duration::milliseconds(50) {
												judge = Judge::Immaculate((delta / Duration::milliseconds(50)) as f32);
											}else if delta < Duration::milliseconds(70) {
												judge = Judge::Extra
											}else if delta < Duration::milliseconds(120) {
												judge = Judge::Normal
											}else if delta < Duration::milliseconds(150) {
												judge = Judge::Fade;
											}else {
												judge = Judge::Miss;
											}

											judge_track.current_judge += 1;
											if show_click_effect {
												let mut shapes = match self.click_effects.get(&note.click_effect_id) {
													Some(t) => t.clone(),
													None => Default::default()
												}.get_shape(&time, &judge, click_effect_position(note, &self.render_queue), &note.note_id);
												self.render_queue.append(&mut shapes);
											}
											self.judged_note_id.push(note.note_id.to_string());
											judges.push(judge);
										},
										(_, JudgeType::Slide) => {
											let judge = Judge::Immaculate(1.0);

											judge_track.current_judge += 1;
											if show_click_effect {
												let mut shapes = match self.click_effects.get(&note.click_effect_id) {
													Some(t) => t.clone(),
													None => Default::default()
												}.get_shape(&time, &judge, click_effect_position(note, &self.render_queue), &note.note_id);
												self.render_queue.append(&mut shapes);
											}
											self.judged_note_id.push(note.note_id.to_string());
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
						for note in notes {
							if note.judge_time < time {
								let judge = Judge::Immaculate(1.0);
								judge_track.current_judge += 1;
								if show_click_effect {
									let mut shapes = match self.click_effects.get(&note.click_effect_id) {
										Some(t) => t.clone(),
										None => Default::default()
									}.get_shape(&time, &judge, click_effect_position(note, &self.render_queue), &note.note_id);
									self.render_queue.append(&mut shapes);
								}
								self.judged_note_id.push(note.note_id.to_string());
								judges.push(judge);
							}
						}
					}
				}
			},
			PlayMode::Replay(_) => todo!()
		}
		for judge in judges {
			self.play_click_sound()?;
			self.caculate(judge);
		}
		Ok(())
	}

	#[inline]
	fn play_click_sound(&mut self) -> Result<(), Error> {
		// if let Some(handle) = &mut self.click_sound_handle {
		// 	if let Err(e) = handle.seek_to(0.0) {
		// 		return Err(PlayError::MusicPlayFailed(kira::manager::error::PlaySoundError::CommandError(e)).into());
		// 	};
		// 	if let Err(e) = handle.resume(Default::default()) {
		// 		return Err(PlayError::MusicPlayFailed(kira::manager::error::PlaySoundError::CommandError(e)).into());
		// 	}
		// }else {
		// 	self.click_sound_handle = match self.audio_manager.play(self.click_sound.clone()) {
		// 		Ok(handle) => Some(handle),
		// 		Err(e) => return Err(PlayError::MusicPlayFailed(e).into()),
		// 	}
		// }
		Ok(())
	}

	#[inline]
	fn caculate(&mut self, judge: Judge) {
		self.judged_notes += 1;
		match judge {
			Judge::Fade | Judge::Miss => self.combo = 0,
			_ => self.combo += 1,
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
	fn get_shape(&self, time: &Duration, judge: &Judge, position: Vec2, note_id: impl Into<String>) -> Vec<Shape> {
		let note_id = note_id.into();
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
			shape.start_time += *time;
			shape.id.clone_from(&note_id);
			for (id, animation) in &mut shape.animation {
				animation.start_time += *time;
				// stupid
				if id == &"----Shape----style----position----x".to_string() || id == &"----Shape----style----position----y".to_string() {
					let delta = if id == &"----Shape----style----position----x".to_string() {
						position.x
					}else {
						position.y
					};
					animation.start_value += delta;
					for linker in &mut animation.linkers {
						linker.end_value += delta;
					}
				}
			}
			shape.shape.move_by(position);
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
			Err(e) => {
				cfg_if::cfg_if! {
					if #[cfg(target_os = "android")] {
						return Ok(log_name)
					}else {
						return Err(e.into())
					}
				}
			}
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
			let supported_extension = [OsStr::new("scc")];
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
				charts.push(ChartInfo::process(path)?)
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
			let supported_extension = [OsStr::new("ssc")];
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
			current_sound: None,
			adjustment: vec!(),
			is_in_delay_adjustment: false,
		}
	}
}