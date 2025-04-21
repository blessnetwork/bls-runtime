use anyhow::{Context, Result, bail};
use blockless::{
    self, BlocklessModule, LoggerLevel, ModuleType, OptimizeOpts, Stderr, Stdin, Stdio, Stdout,
};
use blockless::{BlocklessConfig, DriverConfig, MultiAddr, Permission};
use json::{self, JsonValue};
use rust_car::reader::{self, CarReader};
use rust_car::utils::{extract_ipld, ipld_write};
use std::env::VarError;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use crate::v86config::V86config;

pub(crate) struct CliConfig(pub(crate) BlocklessConfig);

pub(crate) enum Config {
    CliConfig(Box<CliConfig>),
    V86config(V86config),
}

impl Config {
    fn root_path(&self) -> Option<&str> {
        match self {
            Config::CliConfig(config) => config.0.fs_root_path_ref(),
            Config::V86config(config) => Some(&config.fs_root_path),
        }
    }
}

struct EnvVar {
    name: String,
    value: String,
}

impl CliConfig {
    fn defaut_logger_file(filen: &OsStr) -> Option<String> {
        let filen = filen.to_str().unwrap().as_bytes();
        let p = match filen.iter().position(|b| *b == b'.') {
            Some(p) => p,
            None => return Some("runtime".to_string()),
        };
        String::from_utf8(filen[..p].to_vec()).ok()
    }

    /// config the wasm file as entry file
    /// current directory as the root path
    pub fn new_with_wasm(wasm_file: impl AsRef<Path>) -> CliConfig {
        let file_path = wasm_file.as_ref();
        let file_name = file_path.file_name().unwrap();
        let log_file = Self::defaut_logger_file(file_name);
        let mut bconf = BlocklessConfig::new(file_path.to_str().unwrap());
        bconf.set_fs_root_path(Some(".".to_string()));
        bconf.set_runtime_logger_level(LoggerLevel::WARN);
        if let Some(log_file) = log_file.as_ref() {
            bconf.set_runtime_logger(Some(format!("{log_file}.log")));
        }
        CliConfig(bconf)
    }

    fn optimize_options(opt_json: &JsonValue) -> Result<OptimizeOpts> {
        let mut opts: OptimizeOpts = OptimizeOpts::default();
        let opt_items = opt_json
            .entries()
            .map(|item| (item.0.to_string(), json::stringify(item.0)))
            .collect::<Vec<_>>();
        opts.config(opt_items)?;
        Ok(opts)
    }

    fn map_dirs(map_dir: &JsonValue) -> anyhow::Result<Vec<(String, String)>> {
        let mut ret = Vec::new();
        if !map_dir.is_null() {
            if !map_dir.is_array() {
                bail!("the map dir item should be array.");
            }
            for obj in map_dir.members() {
                let host = obj["host"]
                    .as_str()
                    .context("host item is not define.")?
                    .to_string();
                let guest = obj["guest"]
                    .as_str()
                    .context("guest item is not define.")?
                    .to_string();
                ret.push((host, guest));
            }
        }
        Ok(ret)
    }

    fn permissions(permission_json: &JsonValue) -> Vec<Permission> {
        match *permission_json {
            JsonValue::Array(ref perms) => perms
                .iter()
                .filter_map(|p| {
                    let p = p.as_str();
                    match p {
                        Some(p) => {
                            let bs = p.as_bytes();
                            if let Ok(addr) = MultiAddr::parse(bs) {
                                if let Ok(schema) = addr.schema() {
                                    Some(Permission {
                                        schema: schema.into(),
                                        url: p.into(),
                                    })
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        None => None,
                    }
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn drivers(driver_json: &JsonValue) -> Vec<DriverConfig> {
        match *driver_json {
            JsonValue::Array(ref drvs_cfg) => {
                let cfgs: Vec<DriverConfig> = drvs_cfg
                    .iter()
                    .map(|c| {
                        let schema = c["schema"].as_str().map(String::from);
                        let path = c["path"].as_str().map(String::from);
                        (schema, path)
                    })
                    .filter(|(s, p)| s.is_some() && p.is_some())
                    .map(|(s, p)| DriverConfig::new(s.unwrap(), p.unwrap()))
                    .collect::<Vec<_>>();
                cfgs
            }
            _ => Vec::new(),
        }
    }

    fn modules(modules: &JsonValue) -> Vec<BlocklessModule> {
        match *modules {
            JsonValue::Array(ref modules_cfg) => modules_cfg
                .iter()
                .map(|c| {
                    let name = c["name"].as_str().map(String::from).unwrap_or_default();
                    let file = c["file"].as_str().map(String::from).unwrap_or_default();
                    let md5 = c["md5"].as_str().map(String::from).unwrap_or_default();
                    let module_type = c["type"]
                        .as_str()
                        .map(ModuleType::parse_from_str)
                        .unwrap_or(ModuleType::Module);
                    BlocklessModule {
                        module_type,
                        name,
                        file,
                        md5,
                    }
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn from_json_string(json_string: String) -> Result<Self> {
        let json_obj = json::parse(&json_string)?;
        let fs_root_path: Option<String> = json_obj["fs_root_path"].as_str().map(String::from);
        let drivers_root_path: Option<String> =
            json_obj["drivers_root_path"].as_str().map(String::from);
        let limited_fuel: Option<u64> = json_obj["limited_fuel"].as_u64();
        let runtime_logger = json_obj["runtime_logger"].as_str().map(String::from);
        let runtime_logger_level = json_obj["runtime_logger_level"]
            .as_str()
            .map(LoggerLevel::from);
        let limited_memory: Option<u64> = json_obj["limited_memory"].as_u64();
        let extensions_path: Option<String> =
            json_obj["extensions_path"].as_str().map(String::from);
        let stdin: Option<&str> = json_obj["stdin"].as_str();
        let stdout: Option<&str> = json_obj["stdout"].as_str();
        let stderr: Option<&str> = json_obj["stderr"].as_str();
        let debug_info: Option<bool> = json_obj["debug_info"].as_bool();
        let run_time: Option<u64> = json_obj["run_time"].as_u64();

        let drvs = Self::drivers(&json_obj["drivers"]);
        let modules = Self::modules(&json_obj["modules"]);
        let perms: Vec<Permission> = Self::permissions(&json_obj["permissions"]);
        let entry: &str = json_obj["entry"].as_str().unwrap();
        let version = json_obj["version"].as_usize();
        let dirs = Self::map_dirs(&json_obj["map_dirs"])?;
        let mut bc = BlocklessConfig::new(entry);
        //if has the optimize item.
        if json_obj["optimize"].is_object() {
            bc.opts = Self::optimize_options(&json_obj["optimize"])?;
        }
        bc.set_map_dirs(dirs);
        bc.set_modules(modules);
        bc.extensions_path(extensions_path);
        bc.set_fs_root_path(fs_root_path);
        bc.drivers(drvs);
        // the set debug mode
        if let Some(b) = debug_info {
            bc.set_debug_info(b);
        }
        if let Some(l) = runtime_logger_level {
            bc.set_runtime_logger_level(l);
        }
        bc.set_permisions(perms);
        bc.set_runtime_logger(runtime_logger);
        bc.set_drivers_root_path(drivers_root_path);
        bc.limited_fuel(limited_fuel);
        bc.limited_memory(limited_memory);
        bc.set_run_time(run_time);
        if let Some(v) = version {
            bc.set_version(v.into());
        }
        let stdin = match stdin {
            Some(s) => {
                if s == "inherit" {
                    Stdin::Inherit
                } else {
                    Stdin::Fixed(s.to_string())
                }
            }
            _ => Stdin::Fixed(String::new()),
        };
        bc.stdio = Stdio {
            stdin,
            stdout: stdio_cfg!(stdout, Stdout, FileName),
            stderr: stdio_cfg!(stderr, Stderr, FileName),
        };
        Ok(CliConfig(bc))
    }

    pub fn from_data(data: String, root_suffix: Option<String>) -> Result<Self> {
        let data = replace_vars(data, root_suffix)?;
        Self::from_json_string(data)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let values = fs::read_to_string(path)?;
        let json_string = replace_vars(values, None)?;
        Self::from_json_string(json_string)
    }
}

fn env_variables(cid: Option<String>) -> Result<Vec<EnvVar>> {
    let mut vars = Vec::new();
    match std::env::var("ENV_ROOT_PATH") {
        Ok(s) => {
            let env_root = s.clone();
            let path: PathBuf = s.into();
            let root_path = path.join(cid.unwrap_or_default());
            let path: String = root_path.to_str().unwrap_or_default().into();
            vars.push(EnvVar {
                name: "$ROOT".to_string(),
                value: path,
            });
            vars.push(EnvVar {
                name: "$ENV_ROOT_PATH".to_string(),
                value: env_root,
            });
        }
        Err(VarError::NotPresent) => {}
        Err(e) => return Err(e.into()),
    }
    Ok(vars)
}

pub(crate) fn replace_vars(json_str: String, cid: Option<String>) -> Result<String> {
    let vars = env_variables(cid)?;
    let mut raw_json = json_str;
    for var in vars {
        raw_json = raw_json.replace(&var.name, &var.value);
    }
    Ok(raw_json)
}

pub(crate) fn load_from_car<T, F>(car_reader: &mut T, call: F) -> Result<Config>
where
    F: Fn(String, Option<String>) -> Result<Config>,
    T: CarReader,
{
    let cid = car_reader.search_file_cid("config.json")?;
    let mut data = Vec::new();
    ipld_write(car_reader, cid, &mut data)?;
    let raw_json = String::from_utf8(data)?;
    let roots = car_reader.header().roots();
    let root_suffix = roots.first().map(|c| c.to_string());
    call(raw_json, root_suffix)
}

#[cfg(test)]
pub(crate) fn load_cli_config_from_car<T>(car_reader: &mut T) -> Result<CliConfig>
where
    T: CarReader,
{
    let rs = load_from_car(car_reader, new_cliconfig);
    rs.map(|r| match r {
        Config::CliConfig(c) => *c,
        _ => unreachable!("can be reach!"),
    })
}

pub(crate) fn load_extract_from_car<F>(f: File, call: F) -> Result<Config>
where
    F: Fn(String, Option<String>) -> Result<Config>,
{
    let mut reader = reader::new_v1(f)?;
    let cfg = load_from_car(&mut reader, call)?;
    let header = reader.header();
    let rootfs = cfg
        .root_path()
        .expect("root path must be config in car file");
    for rcid in header.roots() {
        let root_path: PathBuf = rootfs.into();
        let root_path = root_path.join(rcid.to_string());
        if !root_path.exists() {
            let msg = format!("create directory error: {}", root_path.to_str().unwrap());
            fs::create_dir(&root_path).expect(&msg);
        }

        extract_ipld(&mut reader, rcid, Some(root_path))?;
    }
    Ok(cfg)
}

fn new_cliconfig(raw_json: String, root_suffix: Option<String>) -> Result<Config> {
    let mut cli_cfg = CliConfig::from_data(raw_json, root_suffix)?;
    cli_cfg.0.set_is_carfile(true);
    Ok(Config::CliConfig(Box::new(cli_cfg)))
}

pub(crate) fn load_cli_config_extract_from_car(f: File) -> Result<CliConfig> {
    let rs = load_extract_from_car(f, new_cliconfig);
    rs.map(|r| match r {
        Config::CliConfig(c) => *c,
        _ => unreachable!("can be reach!"),
    })
}

#[cfg(test)]
mod test {
    #![allow(unused)]
    use std::ffi::OsString;

    use blockless::BlocklessConfigVersion;

    use super::*;

    #[test]
    fn test_defaut_logger_file() {
        let filen: OsString = "test.wasm".into();
        let filen = CliConfig::defaut_logger_file(&filen);
        let filen = filen.unwrap();
        assert_eq!(filen, "test".to_string());

        let filen: OsString = "test".into();
        let filen = CliConfig::defaut_logger_file(&filen);
        let filen = filen.unwrap();
        assert_eq!(filen, "runtime".to_string());
    }

    #[test]
    fn test_new_with_wasm() {
        let cliconf = CliConfig::new_with_wasm("test.wasm");
        let current = Some(".");
        let root_path = cliconf.0.fs_root_path_ref();
        assert_eq!(root_path, current);
        let config_logger_ref = cliconf.0.runtime_logger_path();
        let logger_ref = Some("./test.log".into());
        assert_eq!(logger_ref, config_logger_ref);

        let logger_level = cliconf.0.runtime_logger_path();
        assert!(matches!(&LoggerLevel::WARN, logger_level));
    }

    #[test]
    fn test_load_config() {
        let data = r#"{
            "version": 1,
            "fs_root_path": "$ENV_ROOT_PATH", 
            "drivers_root_path": "$ENV_ROOT_PATH/drivers", 
            "runtime_logger": "runtime.log", 
            "limited_fuel": 200000000,
            "limited_memory": 30,
            "debug_info": false,
            "entry": "release",
            "modules": [
                {
                    "file": "$ROOT/lib.wasm",
                    "name": "lib",
                    "type": "module",
                    "md5": "d41d8cd98f00b204e9800998ecf8427e"
                },
                {
                    "file": "$ROOT/release.wasm",
                    "name": "release",
                    "type": "entry",
                    "md5": "d41d8cd98f00b204e9800998ecf8427e"
                }
            ],
            "permissions": [
                "http://httpbin.org/anything",
                "file://a.go"
            ]
        }"#
        .to_string();

        unsafe {
            std::env::set_var("ENV_ROOT_PATH", "target");
        }
        let config = CliConfig::from_data(data, None).unwrap();
        assert!(matches!(
            config.0.version(),
            BlocklessConfigVersion::Version1
        ));
        assert_eq!(config.0.modules_ref().len(), 2);
    }

    #[test]
    fn test_from_json() {
        let data = r#"{
            "fs_root_path": "/", 
            "drivers_root_path": "/drivers", 
            "runtime_logger": "runtime.log", 
            "limited_fuel": 200000000,
            "limited_memory": 30,
            "debug_info": false,
            "entry": "lib.wasm",
            "permissions": [
                "http://httpbin.org/anything",
                "file://a.go"
            ]
        }"#
        .to_string();
        let config = CliConfig::from_json_string(data).unwrap();
        assert!(matches!(
            config.0.version(),
            BlocklessConfigVersion::Version0
        ));
        assert_eq!(config.0.get_limited_memory(), Some(30));
        assert_eq!(config.0.get_limited_fuel(), Some(200000000));
    }

    #[test]
    fn test_stdin_from_json() {
        let bls_config = CliConfig::from_json_string(
            r#"{
                "fs_root_path": "/", 
                "drivers_root_path": "/drivers", 
                "runtime_logger": "runtime.log", 
                "limited_fuel": 200000000,
                "limited_memory": 30,
                "debug_info": false,
                "entry": "lib.wasm",
                "permissions": []
            }"#
            .to_string(),
        )
        .unwrap()
        .0;
        assert_eq!(bls_config.fix_stdin_ref(), Some(""));

        let bls_config = CliConfig::from_json_string(
            r#"{
                "stdin": "test",
                "fs_root_path": "/", 
                "drivers_root_path": "/drivers", 
                "runtime_logger": "runtime.log", 
                "limited_fuel": 200000000,
                "limited_memory": 30,
                "debug_info": false,
                "entry": "lib.wasm",
                "permissions": []
            }"#
            .to_string(),
        )
        .unwrap()
        .0;
        assert_eq!(bls_config.fix_stdin_ref(), Some("test"));
    }
}
