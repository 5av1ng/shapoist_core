//! The core functions and objects of shapoist
//!
//! Normally you will start from [`ShapoistCore`]

use shapoist_request::prelude::*;
use std::thread::JoinHandle;
use crate::system::command::Command;
use kira::manager::AudioManager;
use crate::system::timer::Timer;
use nablo_shape::shape::animation::StyleToAnimate;
use nablo_shape::math::Area;
use nablo_shape::shape::animation::Animation;
use nablo_shape::shape::Shape;
use std::collections::HashSet;
use std::fmt;
use nablo_shape::math::Vec2;
use std::ops::Range;
use std::path::PathBuf;
use std::collections::HashMap;

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
	pub(crate) thread_pool: Vec<JoinHandle<Result<(), ClientError>>>
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
	/// the first [`String`] stands for script name
	pub chart_varibles: HashMap<String, Vec<Varible>>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
#[serde(default)]
/// define what bpm is and how to link bpm groups (if any).
///
/// # Panics
/// self.len() != linker.len + 1
pub struct Bpm {
	/// the bpm label
	pub bpm: Vec<f32>,
	/// contains when and how to change bpm, time save as nano sec.
	pub linker: Vec<(i64, BpmLinker)>,
}

impl Default for Bpm {
	fn default() -> Self {
		Self {
			bpm: vec!(150.0),
			linker: vec!()
		}
	}
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// define how to link bpms
pub enum BpmLinker {
	Bezier(Vec2, Vec2),
	#[default] Line,
	Power(f32)
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
	pub judge_field: HashMap<String,JudgeField>,
	/// saves shapes, [`String`] stands id of the shape.
	pub shape: HashMap<String, Shape>,
	/// click effects, [`String`] stands id of the effect.
	pub click_effect: HashMap<String, ClickEffect>,
	/// groups, when any elements inside a group is selected, a whole group will be selected
	pub group: Vec<HashSet<Select>>,
	/// how large will the chart take.
	pub size: Vec2
}

impl Default for Chart {
	fn default() -> Self {
		Self {
			notes: HashMap::new(),
			judge_field: HashMap::new(),
			shape: HashMap::new(),
			click_effect: HashMap::new(),
			group: vec!(),
			size: Vec2::new(1920.0,1080.0),
		}
	}
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// stands for notes
pub struct Note {
	/// how to judge this note?
	pub judge_type: JudgeType,
	/// when should player click this note? saves as nano sec.
	pub judge_time: i64, 
	/// which judge field is this note link to?
	pub judge_field_id: String,
	/// which click effect should we use?
	pub click_effect_id: String,
	/// where should we display click effect?
	pub click_effect_position: Vec2,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// what kind of [`Note`] is this?
pub enum JudgeType {
	#[default] Tap,
	Slide,
	Flick,
	/// contains how long should the player hold. saves as nano sec.
	Hold(i64),
	TapAndFlick,
	/// contains where and after when the player moves to next judge field. saves as nano sec. not available in PC.
	Chain(Vec<(String, i64)>),
	/// contains where and after when the player moves to next judge field. saves as nano sec. not available in PC.
	TapChain(Vec<(String, i64)>),
	/// contains which angle should player filck to, save as rad.
	AngledFilck(f32),
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// a judge field
pub struct JudgeField {
	pub area: Area,
	pub animation: HashMap<StyleToAnimate, Animation>
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// a click effect, save as delta value.
pub struct ClickEffect {
	effect: Vec<Shape>
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Eq, Hash)]
/// what was we select?
pub enum Select {
	Note(String),
	JudgeField(String),
	Shape(String),
	ClickEffect(String)	
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
	pub now_select: Select,
	/// changes will fist apply to this after now_select
	pub multi_select: Vec<Select>,
	/// for scripts to select element
	pub label_select: HashMap<String, Vec<Select>>
}

/// contains detailed information while playing
pub struct PlayInfo {
	/// [`Vec<Note>`] is sorted by click_time, note will be kicked out if have judged. auto mode and replay mode this will be empty.
	pub notes_and_judge_field: HashMap<String, Vec<Note>>,
	/// this is sorted by start_time
	pub shape: Vec<Shape>,
	/// shape is rendering, if shape is no longer need to be rendered, it will be kicked out. shape need to be rendered should push into this set
	pub render_queue: Vec<Shape>,
	/// where we render?
	pub current_render: usize,
	pub score: f32,
	pub accuracy: f32,
	pub combo: usize,
	pub max_combo: usize,
	/// how to judge? in editor this will be auto.
	pub play_mode: PlayMode,
	/// the replay file.
	pub replay: Replay,
	/// where the audio plays
	pub audio_manager: AudioManager
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
	pub judge_vec: Vec<Judge>,
	pub score_history: Vec<f32>,
	pub judge_history_events: Vec<JudgeEvent>
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
/// The judgement of a [`Note`]
pub enum Judge {
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
	pub keypresses: Vec<KeyPress>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// is any click? contains both mouse clicks and touch
pub struct Click {
	pub id: usize,
	pub position: Vec2,
	pub is_click: bool
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
/// is key press?
pub struct KeyPress {
	pub id: usize,
	/// numbers contain both numpad and key
	pub key: Key,
	pub is_click: bool
}

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
		}
	}
}