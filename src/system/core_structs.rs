//! The core functions and objects of shapoist
//!
//! Normally you will start from [`ShapoistCore`]

use kira::sound::static_sound::StaticSoundHandle;
use kira::sound::static_sound::StaticSoundData;
use nablo_shape::prelude::shape_elements::Circle;
use nablo_shape::prelude::shape_elements::Color;
use serde::Deserializer;
use serde::Deserialize;
use serde::Serializer;
use nablo_shape::prelude::shape_elements::Rect;
use nablo_shape::prelude::*;
use nablo_data::CanBeAnimated;
use time::Duration;
use shapoist_request::prelude::*;
use std::thread::JoinHandle;
use crate::system::command::Command;
use kira::manager::AudioManager;
use crate::system::timer::Timer;
use nablo_shape::math::Area;
use nablo_shape::shape::animation::Animation;
use nablo_shape::shape::Shape as NabloShape;
use std::collections::HashSet;
use std::fmt;
use nablo_shape::math::Vec2;
use std::ops::Range;
use std::path::PathBuf;
use std::collections::HashMap;
use nablo_shape::prelude::shape_elements::Style as NabloStyle;

const IMMACULATE_COLOR: [u8; 4] = [246, 239, 80, 255];
const EXTRA_COLOR: [u8; 4] = [67, 187, 252, 255];
const NORMAL_COLOR: [u8; 4] = [21, 223, 112, 255];
const FADE_COLOR: [u8; 4] = [107, 55, 34, 255];
const MISS_COLOR: [u8; 4] = [255, 255, 255, 255];

/// The core part of shapoist.
///
/// Normally you would like to start with [`ShapoistCore::new()`], and you should call [`ShapoistCore::frame()`] every time your app updates
pub struct ShapoistCore {
	/// where is the assets floor?
	pub assets_path: PathBuf,
	/// where should we write out log?
	pub log_path: PathBuf,
	/// readed charts
	pub chart_list: Vec<ChartInfo>,
	/// [`Option::None`] for not load one.
	pub current_chart: Option<(Chart, ChartInfo)>,
	/// [`Option::None`] for not load one.
	pub chart_editor: Option<ChartEditor>,
	/// readed scripts.
	pub script_list: Vec<ScriptInfo>,
	/// for scripts, first [`String`] is the name of script, the second presents some data, can be json by using string type.
	pub temp: HashMap<String, Vec<Varible>>,
	/// saves network information
	pub network: NetWork,
	/// the detailed infomation while playing, None for not playing.
	pub play_info: Option<PlayInfo>,
	/// events such as click or rotate.
	pub judge_event: Vec<JudgeEvent>,
	/// timer 
	pub timer: Timer,
	/// save what we have ran, useful for undo command. the max length of this value is setted by [`Settings`]
	pub command_history: Vec<Command>,
	/// settings of shapoist
	pub settings: Settings,
	/// check wheather we are in delay adjustment
	pub is_in_delay_adjustment: bool,
	pub(crate) thread_pool: Vec<JoinHandle<Result<(), ClientError>>>,
	pub(crate) current_sound: Option<StaticSoundData>, 
	pub(crate) adjustment: Vec<Duration>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// stands for vars used by script
pub enum Varible {
	Number(f32),
	Int(isize),
	NumberWithRange(f32, Range<f32>),
	String(String),
	Boolean(bool),
	#[default] None
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// saves detailed information for [`Chart`]
pub struct ChartInfo {
	pub song_name: String,
	pub bpm: Bpm,
	pub diffculty: Diffculty,
	pub producer: String,
	pub charter: String,
	pub artist: String,
	pub version: Version,
	pub path: PathBuf,
	pub image_size: (usize, usize),
	/// different from used_script
	pub needed_scrpt: Vec<ScriptChart>,
	pub used_script: Vec<ScriptChart>,
	pub label: Vec<String>,
	/// None for local chart
	pub publish_info: Option<Publish>,
	pub condition: Condition,
	/// how does this chart len?
	#[serde(serialize_with = "serialize_duration")]
	#[serde(deserialize_with = "deserialize_duration")]
	pub sustain_time: Duration,
	/// the first [`String`] stands for script name
	pub chart_varibles: HashMap<String, Vec<Varible>>,
	pub history: Option<ChartHistory>,
	/// offset time
	#[serde(serialize_with = "serialize_duration")]
	#[serde(deserialize_with = "deserialize_duration")]
	pub offset: Duration,
}

fn serialize_duration<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> 
	where S: Serializer 
{
	let mut serde_state = serde::Serializer::serialize_struct(serializer, "Duration", 1)?;
	serde::ser::SerializeStruct::serialize_field(&mut serde_state, "time", &(duration.as_seconds_f32() * 1e3))?;
	serde::ser::SerializeStruct::end(serde_state)
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)]
struct DeDuration {
	time: f32,
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error> 
	where D: Deserializer<'de>
{
	let de = DeDuration::deserialize(deserializer)?;
	Ok(Duration::seconds_f32(de.time / 1e3))
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// the basic render unit
pub struct Shape {
	pub id: String,
	pub animation: HashMap<String, Animation>,
	pub linked_note_id: Option<Vec<String>>,
	pub shape: NabloShape,
	#[serde(serialize_with = "serialize_duration")]
	#[serde(deserialize_with = "deserialize_duration")]
	pub start_time: Duration,
	#[serde(serialize_with = "serialize_duration")]
	#[serde(deserialize_with = "deserialize_duration")]
	pub sustain_time: Duration,
}

impl CanBeAnimated<'_, NabloShape> for Shape {
	fn get_animation_map(&mut self) -> &mut HashMap<String, Animation> { &mut self.animation }
	fn get_animate_target(&mut self) -> &mut NabloShape { &mut self.shape }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// saves play info
pub struct ChartHistory {
	pub high_score: usize,
	pub high_accurcy: f32,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
#[serde(default)]
/// define what bpm is and how to link bpm groups (if any).
///
/// # Panics
/// self.len() != linker.len + 1
pub struct Bpm {
	/// the bpm label
	pub start_bpm: f32,
	/// contains when and how to change bpm, time save as nano sec.
	pub linkers: Vec<BpmLinker>,
}

impl Default for Bpm {
	fn default() -> Self {
		Self {
			start_bpm: 150.0,
			linkers: vec!()
		}
	}
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// define how to link bpms
pub enum BpmLinkerType {
	Bezier(Vec2, Vec2),
	Linear,
	#[default] Mutation,
	Power(f32)
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
/// define how to link bpms
pub struct BpmLinker {
	#[serde(serialize_with = "serialize_duration")]
	#[serde(deserialize_with = "deserialize_duration")]
	pub time: Duration,
	pub linker: BpmLinkerType,
	pub bpm: f32
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
#[serde(default)]
/// a version sign follows rust version standard
pub struct Version {
	pub major: usize,
	pub minor: usize,
	pub patch: usize,
}

impl Default for Version {
	fn default() -> Self {
		Self {
			major: 0,
			minor: 1,
			patch: 0,
		}
	}
}

impl fmt::Display for Version {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "({}, {}, {})", self.major, self.minor, self.patch)
	}
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
/// what script does this chart use?
pub enum ScriptChart {
	Published(Publish),
	Local(PathBuf),
}

impl Default for ScriptChart {
	fn default() -> Self { 
		ScriptChart::Local(PathBuf::new()) 
	}
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
/// what standard of diffculty measure are we use?
pub enum Diffculty {
	Shapoist(f32, f32),
	Other(String)
}

impl Default for Diffculty {
	fn default() -> Self {
		Self::Shapoist(2.0,2.0)
	}
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
#[serde(default)]
/// a abstrction of chart
pub struct Chart {
	/// saves notes, [`String`] stands id of the note.
	pub notes: HashMap<String, Note>,
	/// saves judge fields, [`String`] stands id of the field.
	pub judge_fields: HashMap<String, JudgeField>,
	/// saves shapes, [`String`] stands id of the shape.
	pub shapes: HashMap<String, Shape>,
	/// saves script objects, [`String`] stands id of the objects. TODO
	pub script_objects: HashMap<String, ()>,
	/// click effects, [`String`] stands id of the effect.
	pub click_effects: HashMap<String, ClickEffect>,
	/// groups, when any elements inside a group is selected, a whole group will be selected
	pub group: Vec<HashSet<Select>>,
	/// how large will the chart take.
	pub size: Vec2,
	/// event when playing.
	pub events: Vec<ChartEvent>,
	/// id and wgsl code
	pub shaders: HashMap<String, String>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct ChartEvent {
	pub inner: ChartEventInner,
	pub time: Duration,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
pub enum ChartEventInner {
	#[default] None,
	ChangeShader(Option<String>),
	Custom(String),
}

impl Default for Chart {
	fn default() -> Self {
		Self {
			notes: HashMap::from([(String::from("default"), Note {
				note_id: "default".into(),
				judge_type: JudgeType::Slide,
				judge_time: Duration::seconds(5),
				judge_field_id: String::from("default"),
				click_effect_id: String::from("default"),
				click_effect_position: Vec2::ZERO,
				linked_shape: None,
			})]),
			judge_fields: HashMap::from([(String::from("default"), JudgeField {
				inner: JudgeFieldInner {
					min: Vec2::ZERO, 
					max: Vec2::same(600.0),
					..Default::default()
				},
				animation: HashMap::new(),
				start_time: Duration::ZERO,
				sustain_time: Duration::seconds(10),
			})]),
			shapes: HashMap::from([(String::from("default"), Shape {
				id: "default".into(),
				animation: HashMap::from([(String::from("----Shape----style----position----y"), Animation {
					start_time: Duration::seconds(4),
					start_value: -100.0,
					linkers: vec!(Linker {
						end_value: 500.0,
						sustain_time: Duration::seconds(1),
						..Default::default()
					})
				})]),
				shape: NabloShape {
					shape: ShapeElement::Rect(Rect {
						width_and_height: Vec2::same(100.0),
						rounding: Vec2::same(10.0),
					}),
					style: NabloStyle {
						clip: Area::INF,
						transform_origin: Vec2::same(500.0),
						..Default::default()
					}
				},
				start_time: Duration::seconds(4),
				sustain_time: Duration::seconds(6),
				linked_note_id: None
			})]),
			click_effects: HashMap::new(),
			script_objects: HashMap::new(),
			group: vec!(),
			// sustain_time: Duration::seconds(10),
			size: Vec2::new(1920.0,1080.0),
			events: vec!(),
			shaders: HashMap::new(),
		}
	}
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// stands for notes
pub struct Note {
	pub note_id: String,
	/// how to judge this note?
	pub judge_type: JudgeType,
	/// when should player click this note?
	#[serde(serialize_with = "serialize_duration")]
	#[serde(deserialize_with = "deserialize_duration")]
	pub judge_time: Duration, 
	/// which judge field is this note link to?
	pub judge_field_id: String,
	/// which click effect should we use?
	pub click_effect_id: String,
	/// where should we display click effect?
	pub click_effect_position: Vec2,
	/// linked shapes, if this is setted, then the click effect will display at center of current shapes, saves as id.
	pub linked_shape: Option<Vec<String>>
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// what kind of [`Note`] is this?
pub enum JudgeType {
	#[default] Tap,
	Slide,
	Flick,
	/// contains how long should the player hold. saves as nano sec.
	Hold(Duration),
	TapAndFlick,
	/// contains where and after when the player moves to next judge field. not available in PC. 
	Chain(Vec<(String, Duration)>),
	/// contains where and after when the player moves to next judge field. not available in PC.
	TapChain(Vec<(String, Duration)>),
	/// contains which angle should player filck to, save as rad.
	AngledFilck(f32),
	AngledTapFilck(f32),
}

/// saves info during judging
pub struct JudgeInfo {
	/// saves where we judge
	pub current_judge: usize,
	/// what we need keep tracking
	pub judge_tracks: Vec<JudgeTrack>
}

/// save which note should we tracing on
pub struct JudgeTrack {
	pub note_id: usize,
	pub linked_click: usize,
	pub start_time: Duration,
	pub last_position: Vec2,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
// #[serde(default)]
/// a judge field
pub struct JudgeField {
	pub inner: JudgeFieldInner,
	pub animation: HashMap<String, Animation>,
	#[serde(serialize_with = "serialize_duration")]
	#[serde(deserialize_with = "deserialize_duration")]
	pub start_time: Duration,
	#[serde(serialize_with = "serialize_duration")]
	#[serde(deserialize_with = "deserialize_duration")]
	pub sustain_time: Duration,
}

impl CanBeAnimated<'_, JudgeFieldInner> for JudgeField {
	fn get_animation_map(&mut self) -> &mut HashMap<String, Animation> { &mut self.animation }
	fn get_animate_target(&mut self) -> &mut JudgeFieldInner { &mut self.inner }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
#[serde(default)]
/// to use CanBeAnimated trait
pub struct JudgeFieldInner {
	pub min: Vec2,
	pub max: Vec2,
	pub position: Vec2,
	pub transform_origin: Vec2,
	pub scale: Vec2,
	pub rotate: f32,
}

impl Default for JudgeFieldInner {
	fn default() -> Self {
		Self {
			min: Vec2::ZERO,
			max: Vec2::ZERO,
			rotate: f32::default(),
			position: Vec2::default(),
			transform_origin: Vec2::default(),
			scale: Vec2::NOT_TO_SCALE,
		}
	}
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
#[serde(default)]
/// a click effect, save as delta value.
pub struct ClickEffect {
	pub immaculate_effect: Vec<Shape>,
	pub extra_effect: Vec<Shape>,
	pub normal_effect: Vec<Shape>,
	pub fade_effect: Vec<Shape>,
	pub miss_effect: Vec<Shape>,
}

impl Default for ClickEffect {
	fn default() -> Self {
		fn get_default_by_color(fill: impl Into<Color>, sustain_time: Duration) -> Vec<Shape> {
			let radius = 75.0;
			let linker = AnimationLinker::Bezier(Vec2::new(0.5, 0.0), Vec2::new(0.5, 1.0));
			vec!(Shape {
				id: "".into(),
				animation: HashMap::from([(String::from("----Shape----style----fill----color----a"), Animation {
					start_time: Duration::ZERO,
					start_value: 255.0,
					linkers: vec!(Linker {
						end_value: 0.0,
						sustain_time,
						linker: linker.clone(),
					})
				}),
				(String::from("----Shape----style----position----x"), Animation {
					start_time: Duration::ZERO,
					start_value: 0.0,
					linkers: vec!(Linker {
						end_value: -radius,
						sustain_time,
						linker: linker.clone(),
					})
				}),
				(String::from("----Shape----style----position----y"), Animation {
					start_time: Duration::ZERO,
					start_value: 0.0,
					linkers: vec!(Linker {
						end_value: -radius,
						sustain_time,
						linker: linker.clone(),
					})
				}),
				(String::from("----Shape----shape----Circle----radius"), Animation {
					start_time: Duration::seconds(0),
					start_value: 0.0,
					linkers: vec!(Linker {
						end_value: radius,
						sustain_time,
						linker,
					})
				})]),
				shape: NabloShape {
					shape: ShapeElement::Circle(Circle { radius: 0.0 }),
					style: NabloStyle {
						clip: Area::INF,
						transform_origin: Vec2::same(0.0),
						fill: fill.into(),
						..Default::default()
					}
				},
				start_time: Duration::ZERO,
				sustain_time,
				linked_note_id: None
			})
		}

		let sustain_time = Duration::seconds_f32(0.3);
		Self {
			immaculate_effect: get_default_by_color(IMMACULATE_COLOR, sustain_time),
			extra_effect: get_default_by_color(EXTRA_COLOR, sustain_time),
			normal_effect: get_default_by_color(NORMAL_COLOR, sustain_time),
			fade_effect: get_default_by_color(FADE_COLOR, sustain_time),
			miss_effect: get_default_by_color(MISS_COLOR, sustain_time),
		}
	}
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Eq, Hash)]
/// what was we select?
pub enum Select {
	Note(String),
	JudgeField(String),
	Shape(String),
	ClickEffect(String),
	// TODO
	Script(()),
}

impl Default for Select {
	fn default() -> Self {
		Self::Note(String::new())
	}
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// contains methods to edit chart
pub struct ChartEditor {
	/// changes will fist apply to this
	pub now_select: Option<Select>,
	/// changes will fist apply to this after now_select
	pub multi_select: Vec<Select>,
	/// saves things that have been cloned(saves relative timer refer to current time.), None for havent clone yet.
	pub clone_buffer: Vec<SelectUnchange>,
	/// will we show click effects?
	pub show_click_effect: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
/// what was we select, and for some reason we need them to keep unchanged after selected? [`String refers to id`]
pub enum SelectUnchange {
	Note(String, Note),
	JudgeField(String, JudgeField),
	Shape(String, Shape),
	ClickEffect(String, ClickEffect),
	// TODO
	Script(String, ()),
}

/// contains detailed information while playing
pub struct PlayInfo {
	/// [`Vec<Note>`] is sorted by click_time, note will be kicked out if have judged. auto mode and replay mode this will be empty.
	pub notes: HashMap<String, Vec<Note>>,
	/// saves judge field and where do we judge.
	pub judge_fields: HashMap<String, (JudgeField, JudgeInfo)>,
	pub click_effects: HashMap<String, ClickEffect>,
	/// this is sorted by start_time
	pub shapes: Vec<Shape>,
	/// shape is rendering, if shape is no longer need to be rendered, it will be kicked out. shape need to be rendered should push into this set
	pub render_queue: Vec<Shape>,
	pub score: f32,
	pub accuracy: f32,
	pub combo: usize,
	pub max_combo: usize,
	/// how to judge? in editor this will be auto.
	pub play_mode: PlayMode,
	/// the replay file.
	pub replay: Replay,
	/// where the audio plays
	pub judge_vec: Vec<Judge>,
	pub audio_manager: AudioManager,
	pub total_notes: usize,
	pub judged_notes: usize,
	pub is_finished: bool,
	pub sustain_time: Duration,
	pub current_render: usize,
	pub offset: Duration,
	pub is_track_played: bool,
	pub track_path: PathBuf,
	#[cfg(target_arch = "wasm32")]
	pub sound_data: Vec<u8>,
	pub(crate) judged_note_id: Vec<String>,
	pub click_sound: StaticSoundData,
	pub click_sound_handle: Option<StaticSoundHandle>,
	/// sort by time
	pub events: Vec<ChartEvent>,
	pub current_event: usize,
	/// none for default, contains shader code
	pub current_shader: Option<String>,
	pub shaders: HashMap<String, String>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// how to judge? in editor this will be auto.
pub enum PlayMode {
	Normal,
	#[default] Auto,
	Replay(Replay)
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// the replay file.
pub struct Replay {
	pub score_history: Vec<f32>,
	pub judge_history_events: Vec<JudgeEvent>
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// The judgement of a [`Note`]
pub enum Judge {
	/// saves (now - click_time) / Duration::milis(50). 
	Immaculate(f32),
	Extra,
	Normal,
	Fade,
	#[default] Miss
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// contains clicks and keypresses
pub struct JudgeEvent {
	/// contains both mouse clicks and touch
	pub clicks: Vec<Click>,
	// pub keypresses: Vec<KeyPress>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// is any click? contains both mouse clicks and touch
pub struct Click {
	pub id: usize,
	pub position: Vec2,
	pub state: ClickState
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// click state
pub enum ClickState {
	Pressed,
	Pressing,
	#[default] Released
}

// #[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
// #[serde(default)]
// /// is key press?
// pub struct KeyPress {
// 	pub id: usize,
// 	/// numbers contain both numpad and key
// 	pub key: Key,
// 	pub is_click: bool
// }

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// Human readable keyname
pub enum Key {
	#[default] A,B,C,D,E,F,G,H,I,J,K,L,M,N,O,P,Q,R,S,T,U,V,W,X,Y,Z,One,Two,Three,Four,Five,Six,Seven,Eight,Nine,Zero,NumPad1,NumPad2,NumPad3,NumPad4,NumPad5,NumPad6,NumPad7,NumPad8,NumPad9,NumPad0,ArrowDown,
	ArrowLeft,ArrowRight,ArrowUp,Escape,Tab,Backspace,Enter,Space,Insert,Delete,Home,End,PageUp,PageDown,Minus,PlusEquals,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// detailed script information
pub struct ScriptInfo {
	pub configs: Vec<ScriptConfig>,
	/// None for not publish
	pub publish_info: Option<Publish>,
	/// top path
	pub path: PathBuf,
	pub condition: Condition,
	pub label: Vec<String>
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// is this source usable?
pub enum Condition {
	Normal,
	Broken,
	#[default] Unknown
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// how to use this script?
pub struct ScriptConfig {
	/// see more detailed on our documentation(TODO).
	pub script_type: ScriptType,
	/// None for not change the name
	pub entrance: Option<HashMap<String, String>>,
	/// second path
	pub path: PathBuf,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default, Eq, Hash)]
/// what type is the script?
pub enum ScriptType {
	/// [`String`] refers to the first code of this script
	Editor(String),
	Render,
	#[default] Logic
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// represents a lua or python script
pub struct Script {
	code: String
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// the detailed source information from server
pub struct Publish {
	pub id: usize,
	pub label: Vec<String>,
	pub description: String,
	/// what type is this source?
	pub publish_type: PublishType,
	pub uploader: usize
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// what type is this source?
pub enum PublishType {
	#[default] Chart,
	Music,
	Script
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// saves struct when connecting to server
pub struct NetWork {
	pub request: Request
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
#[serde(default)]
/// the settings of shapoist
pub struct Settings {
	/// how many notes can be judged at the same time? by default its 10.
	pub search_depth: usize,
	/// read from settings, active order is exact the [`Vec`]'s order.
	pub enabled_script: HashMap<ScriptType, Vec<(Script, ScriptInfo)>>,
	/// how long should we remain logs? by default it is 5 days
	pub log_remain: std::time::Duration,
	/// need we check chart condition when creating new [`ShapoistCore`]? by default its true
	pub need_check_chart: bool,
	/// need we check script condition when creating new [`ShapoistCore`]? by default its true
	pub need_check_script: bool,
	/// how many thread should we take while communicating to server? by default its 5. this will affect how many file can dowload once a time
	pub thread_handels: usize,
	/// how many commands should we save as history? by default its 30
	pub command_history: usize,
	/// normalized
	pub music_volume: f32,
	/// normalized
	pub click_sound_volume: f32,
	/// offset
	#[serde(serialize_with = "serialize_duration")]
	#[serde(deserialize_with = "deserialize_duration")]
	pub offset: Duration,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			search_depth: 10,
			enabled_script: HashMap::new(),
			log_remain: std::time::Duration::new(43200, 0),
			need_check_chart: true,
			need_check_script: true,
			thread_handels: 4,
			command_history: 30,
			music_volume: 0.8,
			click_sound_volume: 0.7,
			offset: Duration::ZERO,
		}
	}
}