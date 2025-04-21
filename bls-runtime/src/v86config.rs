use std::fs::File;

use crate::{
    config::{Config, load_extract_from_car, replace_vars},
    error::CliExitCode,
};
use anyhow::Result;

pub(crate) struct V86config {
    pub raw_config: Option<String>,
    pub fs_root_path: String,
    pub dynamic_lib_path: String,
}

#[allow(dead_code)]
impl V86config {
    fn from_json_string(json_string: String) -> Result<Self> {
        let json_obj = json::parse(&json_string)?;
        let fs_root_path: Option<String> = json_obj["fs_root_path"].as_str().map(String::from);
        let fs_root_path = fs_root_path.unwrap();

        let dynamic_lib_path: Option<String> =
            json_obj["dynamic_lib_path"].as_str().map(String::from);
        let dynamic_lib_path = dynamic_lib_path.unwrap();
        Ok(V86config {
            fs_root_path,
            dynamic_lib_path,
            raw_config: None,
        })
    }

    pub fn from_data(data: String, root_suffix: Option<String>) -> Result<Self> {
        let data = replace_vars(data, root_suffix)?;
        Self::from_json_string(data)
    }

    pub fn from_file(data: String, root_suffix: Option<String>) -> Result<Self> {
        let data = replace_vars(data, root_suffix)?;
        Self::from_json_string(data)
    }
}

pub(crate) fn load_v86conf_extract_from_car(f: File) -> Result<V86config, CliExitCode> {
    let config = load_extract_from_car(f, |raw_json, root_suffix| {
        let mut cfg = V86config::from_data(raw_json.clone(), root_suffix.clone())?;
        cfg.raw_config = replace_vars(raw_json, root_suffix).ok();
        Ok(Config::V86config(cfg))
    })
    .map_err(|err| CliExitCode::UnknownError(err.to_string()))?;

    match config {
        Config::V86config(cfg) => Ok(cfg),
        _ => Err(CliExitCode::UnknownError(
            "the v86 car file does not exist or is unreadable.".to_string(),
        )),
    }
}

#[cfg(test)]
mod test {
    use super::V86config;

    #[test]
    fn test_v86_config() {
        let data = r#"{
            "fs_root_path": "$ENV_ROOT_PATH", 
            "dynamic_lib_path": "$ROOT/test.so"
        }
        "#;
        unsafe {
            std::env::set_var("ENV_ROOT_PATH", "/temp/v86");
        }
        let cfg = V86config::from_data(data.into(), Some("1".into()));
        let cfg = cfg.unwrap();
        assert_eq!(&cfg.dynamic_lib_path, "/temp/v86/1/test.so");
    }

    #[test]
    fn test_v86_config2() {
        let data = r#"{
            "fs_root_path": "$ENV_ROOT_PATH", 
            "dynamic_lib_path": "$ROOT/test.so"
        }
        "#;
        unsafe {
            std::env::set_var("ENV_ROOT_PATH", "/temp/v86");
        }
        let cfg = V86config::from_data(data.into(), None);
        let cfg = cfg.unwrap();
        assert_eq!(&cfg.dynamic_lib_path, "/temp/v86//test.so");
    }
}
