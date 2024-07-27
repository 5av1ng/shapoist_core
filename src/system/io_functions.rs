use std::path::Path;
use crate::system::Error;
use std::io::{Write, BufRead, Read};
use std::{io, fs};
use log::*;

#[allow(dead_code)]
pub(crate) fn create_dir<P: AsRef<Path>>(path: P) -> Result<(), Error>{
	debug!("creating dir {}", path.as_ref().display());
	match fs::create_dir(&path){
		Ok(_) => {
			info!("dir {} created", path.as_ref().display());
			Ok(())
		},
		Err(e) => {
			error!("creating dir failed. info: {}", e);
			Err(e.into())
		}
	}
}

#[allow(dead_code)]
pub(crate) fn create_file<P: AsRef<Path>>(path: P) -> Result<(), Error>{
	debug!("creating file {}", path.as_ref().display());
	match fs::File::create(&path){
		Ok(_) => {
			info!("file {} created", path.as_ref().display());
			Ok(())
		},
		Err(e) => {
			error!("creating file failed. info: {}", e);
			Err(e.into())
		}
	}
}

#[allow(dead_code)]
pub(crate) fn write_file<P: AsRef<Path>>(path: P, mut input: impl Read) -> Result<(), Error>{
	debug!("writing file {}", path.as_ref().display());
	// if i set append to false, i will get os error 87, but i dont know why
	remove_file(&path)?;
	create_file(&path)?;
	let mut buffer = vec!();
	if let Err(e) = input.read_to_end(&mut buffer) {
		error!("opening file failed, info: {}", e);
		return Err(e.into())
	};
	let mut file = match fs::OpenOptions::new().append(true).open(&path) {
		Ok(t) => t,
		Err(e) => {
			error!("opening file failed, info: {}", e);
			return Err(e.into());
		}
	};
	match file.write_all(buffer.as_slice()){
		Ok(_) => {
			debug!("file {} have written", path.as_ref().display());
			info!("file {} written", path.as_ref().display());
			Ok(())
		},
		Err(e) => {
			error!("writing file failed, info: {}", e);
			Err(e.into())
		}
	}
}

// pub(crate) fn read_file_split<P: AsRef<Path>>(path: P) -> Result<Vec<String>, Error>{
// 	let file_open = fs::File::open(&path);
// 	match file_open {
// 		Ok(file) => {
// 			let file_open = io::BufReader::new(file).lines();
// 			let mut file_lines = Vec::new();
// 			for line in file_open{
// 				if let Ok(data) = line {
// 					file_lines.push(data);
// 				}
// 			};
// 			return Ok(file_lines);
// 		},
// 		Err(e) => {
// 			return Err(e.into());
// 		},
// 	}
// }

#[allow(dead_code)]
pub(crate) fn read_file_to_string<P: AsRef<Path>>(path: P) -> Result<String, Error>{
	debug!("reading file {}", path.as_ref().display());
	let file_open = fs::File::open(&path);
	match file_open {
		Ok(file) => {
			debug!("convering into string..");
			let file_open = io::BufReader::new(file).lines();
			let mut file_lines_collect = String::new();
			for line in file_open.map_while(Result::ok){
				file_lines_collect = file_lines_collect + "\n" + &line;
			};
			info!("file {} read into string", path.as_ref().display());
			Ok(file_lines_collect)
		},
		Err(e) => {
			error!("reading file failed, info: {}", e);
			Err(e.into())
		},
	}
}

#[allow(dead_code)]
pub(crate) fn read_file<P: AsRef<Path>>(path: P) -> Result<std::fs::File, Error>{
	debug!("reading file {}", path.as_ref().display());
	let file_open = fs::File::open(&path);
	match file_open {
		Ok(file) => {
			info!("file {} read", path.as_ref().display());
			Ok(file)
		},
		Err(e) => {
			error!("reading file failed, info: {}", e);
			Err(e.into())
		},
	}
}

#[allow(dead_code)]
pub(crate) fn remove_file<P: AsRef<Path>>(path: P) -> Result<(), Error> {
	debug!("removing file {}", path.as_ref().display());
	match fs::remove_file(&path){
		Ok(_) => {
			info!("file {} removed", path.as_ref().display());
			Ok(())
		},
		Err(e) => {
			error!("removing file failed, info: {}", e);
			Err(e.into())
		},
	}
}

#[allow(dead_code)]
pub(crate) fn remove_path<P: AsRef<Path>>(path: P) -> Result<(), Error> {
	debug!("removing path {}", path.as_ref().display());
	match fs::remove_dir_all(&path){
		Ok(_) => {
			info!("path {} removed", path.as_ref().display());
			Ok(())
		},
		Err(e) => {
			error!("removing path failed, info: {}", e);
			Err(e.into())
		},
	}
}

#[allow(dead_code)]
pub(crate) fn read_every_file<P: AsRef<Path>>(path: P) -> Result<Vec<String>, Error>{
	let path_read = fs::read_dir(path);
	let mut vec_back = Vec::new();
	match path_read {
		Ok(t) => {
			for path in t {
				match path {
					Ok(sth) => vec_back.push(sth.path().display().to_string()),
					Err(err) => return Err(err.into()),
				}
			}
			Ok(vec_back)
		},
		Err(e) => {
			Err(e.into())
		},
	}
}

#[allow(dead_code)]
pub(crate) fn read_metadata<P: AsRef<Path>>(path: P) -> Result<fs::Metadata, Error>{
	match fs::metadata(path) {
		Ok(t) => Ok(t),
		Err(e) => Err(e.into()),
	}
}

#[allow(dead_code)]
pub(crate) fn copy_file<P: AsRef<Path>>(file_path: P, copy_path: P) -> Result<(), Error>{
	match fs::copy(file_path, copy_path) {
		Ok(_) => {
			Ok(())
		},
		Err(e) => {
			Err(e.into())
		},
	}
}

#[allow(dead_code)]
pub(crate) fn to_toml<T: serde::Serialize>(input: &T) -> Result<String, Error> {
	match toml::to_string_pretty(input) {
		Ok(t) => Ok(t),
		Err(e) => {
			Err(Error::ConvertToError(e))
		}
	}
}

#[allow(dead_code)]
pub(crate) fn parse_toml<T: for<'a> serde::Deserialize<'a>>(input: &str) -> Result<T, Error> {
	match toml::from_str(input) {
		Ok(t) => Ok(t),
		Err(e) => {
			Err(Error::ConvertInError(e))
		}
	}
}