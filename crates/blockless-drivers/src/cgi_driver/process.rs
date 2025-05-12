use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Mutex, MutexGuard},
};

use crate::CgiErrorKind;
use json::JsonValue;
use json::object::Object as JsonObject;
use log::{debug, error};
#[cfg(target_family = "unix")]
use std::os::unix::prelude::MetadataExt;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
};

const DB_NAME: &str = ".extsdb";

use super::db::{DB, ExtensionMeta, ExtensionMetaStatus};

fn get_db(path: impl AsRef<Path>) -> MutexGuard<'static, Option<DB>> {
    static mut DB: Mutex<Option<DB>> = Mutex::new(None);
    unsafe {
        let mut db = DB.lock().unwrap();
        if db.is_none() {
            db.replace(
                DB::new(path)
                    .map_err(|e| error!("error open db {}", e))
                    .ok()
                    .unwrap(),
            );
            db.as_mut().and_then(|db| {
                if db.create_schema().is_ok() {
                    Some(db)
                } else {
                    None
                }
            });
        }
        db
    }
}

pub struct CgiProcess {
    root_path: String,
    child: Option<Child>,
    command: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
}

impl CgiProcess {
    /// create a CgiProcess with arguments and envriment variables .
    pub fn new(root_path: String, cmd_with_params: &str) -> Result<Self, CgiErrorKind> {
        let obj = match json::parse(cmd_with_params) {
            Ok(o) => o,
            Err(_) => return Err(CgiErrorKind::InvalidParameter),
        };
        let command = match obj["command"].as_str() {
            Some(s) => String::from(s),
            None => return Err(CgiErrorKind::InvalidParameter),
        };

        let command = match get_command_with_alias(&root_path, &command) {
            Some(c) => c.file_name,
            None => return Err(CgiErrorKind::InvalidExtension),
        };

        let args = match obj["args"] {
            json::JsonValue::Array(ref args) => args
                .iter()
                .filter_map(|arg| arg.as_str().map(String::from))
                .collect(),
            _ => Vec::new(),
        };
        let envs = match obj["envs"] {
            json::JsonValue::Array(ref args) => args
                .iter()
                .filter_map(|arg| {
                    arg["env_name"]
                        .as_str()
                        .map(String::from)
                        .zip(arg["env_val"].as_str().map(String::from))
                })
                .collect(),
            _ => Vec::new(),
        };
        Ok(Self {
            child: None,
            root_path,
            command,
            args,
            envs,
        })
    }

    /// read bytes from the stdout .
    pub async fn child_stdout_read(&mut self, buf: &mut [u8]) -> Result<u32, CgiErrorKind> {
        if self.child.is_some() {
            let child = self.child.as_mut().unwrap();
            if child.stdout.is_some() {
                let stdout = child.stdout.as_mut().unwrap();
                return stdout.read(buf).await.map(|i| i as u32).map_err(|e| {
                    debug!("error read stdout {}", e);
                    CgiErrorKind::RuntimeError
                });
            } else {
                return Ok(0);
            }
        }
        Err(CgiErrorKind::InvalidHandle)
    }

    /// read bytes from the stderr .
    pub async fn child_stderr_read(&mut self, buf: &mut [u8]) -> Result<u32, CgiErrorKind> {
        if self.child.is_some() {
            let child = self.child.as_mut().unwrap();
            if child.stderr.is_some() {
                let stderr = child.stderr.as_mut().unwrap();
                return stderr.read(buf).await.map(|i| i as u32).map_err(|e| {
                    debug!("error read stderr {}", e);
                    CgiErrorKind::RuntimeError
                });
            } else {
                return Ok(0);
            }
        }
        Err(CgiErrorKind::InvalidHandle)
    }

    /// write buf bytes to the stdin .
    pub async fn child_stdin_write(&mut self, buf: &[u8]) -> Result<u32, CgiErrorKind> {
        if self.child.is_some() {
            let child = self.child.as_mut().unwrap();
            if child.stdin.is_some() {
                let stdin = child.stdin.as_mut().unwrap();
                return stdin.write(buf).await.map(|i| i as u32).map_err(|e| {
                    debug!("error write stdin {}", e);
                    CgiErrorKind::RuntimeError
                });
            } else {
                return Ok(0);
            }
        }
        Err(CgiErrorKind::InvalidHandle)
    }

    /// kill the children process .
    #[allow(unused)]
    pub async fn kill(&mut self) -> Result<(), CgiErrorKind> {
        if self.child.is_some() {
            return self
                .child
                .as_mut()
                .unwrap()
                .kill()
                .await
                .map_err(|_| CgiErrorKind::RuntimeError);
        }
        Err(CgiErrorKind::InvalidHandle)
    }

    /// the extern to exec the command with the arguments and envoriment variables .
    pub fn exec(&mut self) -> Result<(), CgiErrorKind> {
        let exec_file = format!("{}/{}", self.root_path, &self.command);
        let mut command = Command::new(&exec_file);
        command.stderr(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stdin(Stdio::piped());
        command.kill_on_drop(true);
        command.args(self.args.iter());
        command.envs(self.envs.clone());
        self.child = match command.spawn() {
            Ok(o) => Some(o),
            Err(e) => {
                debug!("error exec command: {}", e);
                return Err(CgiErrorKind::RuntimeError);
            }
        };
        Ok(())
    }
}

/// get db file name from path.
fn get_db_file_name(path: &str) -> PathBuf {
    let path = Path::new(path);
    path.join(DB_NAME)
}

fn list_extensions_from_db(path: &str) -> HashMap<String, ExtensionMeta> {
    let db_file_name = get_db_file_name(path);
    let mut db = get_db(db_file_name);
    //load the metas from db
    //the ids in db will use for delete the invalid extension.
    db.as_mut()
        .map(|db| {
            let exts = db
                .list_extensions()
                .map_err(|e| error!("error list extensions: {}", e))
                .unwrap_or_default();
            let mut exts_metas = HashMap::new();
            for mut ext in exts.into_iter() {
                ext.status = ExtensionMetaStatus::Invalid;
                exts_metas.insert(ext.file_name.clone(), ext);
            }
            exts_metas
        })
        .unwrap_or_default()
}

/// get the file md5 summary
async fn file_md5(path: impl AsRef<Path>) -> anyhow::Result<md5::Digest> {
    let mut file = File::open(path).await?;
    let mut buf = [0u8; 4096];
    let mut md5_ctx = md5::Context::new();
    loop {
        let rn = file.read(&mut buf).await?;
        if rn == 0 {
            break;
        }
        md5_ctx.consume(&buf[..rn]);
    }
    Ok(md5_ctx.compute())
}

/// get file meta from execute output with the parameter "--ext_verify".
async fn get_file_meta(file_path: &str) -> anyhow::Result<Option<ExtensionMeta>> {
    let mut command = Command::new(file_path);
    command.args(["--ext_verify"]);
    let child = command.output().await?;
    let val = std::str::from_utf8(&child.stdout[..])?;
    //parse output json like {"alias":"xxx", "md5":"xxxx", "description":"xxxxx", "is_cgi": true}
    let json = json::parse(val.trim())?;
    let is_cgi = match json["is_cgi"].as_bool() {
        Some(b) => b,
        None => return Ok(None),
    };
    if !is_cgi {
        return Ok(None);
    }
    let alias = match json["alias"].as_str() {
        Some(b) => b.into(),
        None => return Ok(None),
    };
    let md5: String = match json["md5"].as_str() {
        Some(md5) => md5.into(),
        None => {
            //TODO: next step the file must be embed the md5
            //value and output with the verify command.
            let file_md5 = file_md5(file_path).await?;
            format!("{:02x}", file_md5)
        }
    };
    let description: String = match json["description"].as_str() {
        Some(d) => d.into(),
        None => return Ok(None),
    };
    Ok(Some(ExtensionMeta {
        md5,
        alias,
        description,
        ..Default::default()
    }))
}

async fn list_cgi_directory(
    path: impl AsRef<Path>,
    meta_db: &mut HashMap<String, ExtensionMeta>,
) -> anyhow::Result<Vec<ExtensionMeta>> {
    let mut read_dir = tokio::fs::read_dir(path).await?;
    let mut rs = Vec::new();
    while let Some(entry) = read_dir.next_entry().await? {
        let file_name = entry.file_name();
        if file_name == DB_NAME {
            continue;
        }
        let meta_data = entry.metadata().await?;
        if !meta_data.is_file() {
            continue;
        }
        #[cfg(target_family = "unix")]
        if meta_data.mode() & 0o111 == 0 {
            continue;
        }
        let full_path = entry.path();
        let md5 = file_md5(&full_path).await?;
        let md5 = format!("{:02x}", md5);
        let file_name: String = entry.file_name().to_str().unwrap_or("").into();
        match meta_db.get_mut(&file_name) {
            Some(meta) => {
                if meta.md5 != md5 {
                    meta.md5 = md5;
                }
                //update sqlite with normal status, there is update status just for flags.
                meta.status = ExtensionMetaStatus::Update;
            }
            None => {
                let fpath: &str = full_path.as_os_str().to_str().unwrap();
                let extend_meta = get_file_meta(fpath).await?;
                if let Some(mut extend_meta) = extend_meta {
                    extend_meta.file_name = file_name;
                    rs.push(extend_meta);
                }
            }
        }
    }
    Ok(rs)
}

async fn cgi_directory_list_extensions(path: &str) -> Result<Vec<ExtensionMeta>, CgiErrorKind> {
    let mut meta_maps = list_extensions_from_db(path);
    let mut metas = list_cgi_directory(path, &mut meta_maps)
        .await
        .map_err(|e| {
            error!("error list_cgi_directory: {}", e);
            CgiErrorKind::InvalidExtension
        })?;
    for val in meta_maps.into_values() {
        metas.push(val);
    }
    let mut db = get_db(path);
    match db.as_mut().map(|db| {
        db.save_extensions(&metas).map_err(|e| {
            error!("save extensions error: {}", e);
            CgiErrorKind::InvalidExtension
        })
    }) {
        Some(Ok(v)) => v,
        Some(Err(e)) => return Err(e),
        None => return Err(CgiErrorKind::RuntimeError),
    };
    let metas = metas
        .into_iter()
        .filter(|meta| !matches!(meta.status, ExtensionMetaStatus::Invalid))
        .collect::<Vec<_>>();
    Ok(metas)
}

/// The CGI must support "--ext_verify" parameter, the runtime will be call with the parameter.
pub async fn cgi_directory_list_exec(path: &str) -> Result<String, CgiErrorKind> {
    let exts = cgi_directory_list_extensions(path).await?;
    let exts: Vec<JsonValue> = exts
        .into_iter()
        .map(|ext| {
            let mut json_obj = JsonObject::new();
            json_obj.insert("fileName", JsonValue::String(ext.file_name));
            json_obj.insert("alias", JsonValue::String(ext.alias));
            json_obj.insert("md5", JsonValue::String(ext.md5));
            json_obj.insert("description", JsonValue::String(ext.description));
            JsonValue::Object(json_obj)
        })
        .collect();
    let vals = JsonValue::Array(exts);
    Ok(json::stringify(vals))
}

fn get_command_with_alias(path: &str, alias: &str) -> Option<ExtensionMeta> {
    match get_db(path)
        .as_mut()
        .map(|db| db.get_extension_by_alias(alias))
    {
        Some(Ok(s)) => s,
        Some(Err(e)) => {
            error!("error get_extension_by_alias: {}", e);
            None
        }
        None => None,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{fs, io::Write, os::unix::prelude::OpenOptionsExt};
    use tempdir::{self, TempDir};
    use tokio_test;
    struct DropDir {
        path: PathBuf,
    }

    impl Drop for DropDir {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    async fn _test_extensions_file(
        temp_dir: &TempDir,
        filename: &str,
    ) -> anyhow::Result<Vec<ExtensionMeta>> {
        let test_extension = temp_dir.path().join(filename);
        {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o700)
                .open(test_extension)
                .unwrap();
            //The CGI must support "--ext_verify" parameter
            let script = format!(
                r#"#!/usr/bin/env sh 
            echo '{{"alias":"{}", "description":"eeeeee", "is_cgi":true}}'"#,
                filename
            );
            file.write_all(script.as_bytes())?;
        }
        let path = temp_dir.path().to_str().unwrap();
        let exts = cgi_directory_list_extensions(path).await?;
        Ok(exts)
    }

    #[test]
    fn test_extensions_file() {
        #[cfg(target_family = "unix")]
        tokio_test::block_on(async {
            let temp_dir = tempdir::TempDir::new("drivers-test").unwrap();
            let drop_dir = DropDir {
                path: temp_dir.path().to_path_buf(),
            };
            let exts = _test_extensions_file(&temp_dir, "f1").await.unwrap();
            assert_eq!(exts.len(), 1);
            assert!(exts[0].alias == "f1");
            assert!(exts[0].description == "eeeeee");
            assert!(exts[0].file_name == "f1");
            let exts = _test_extensions_file(&temp_dir, "f2").await.unwrap();
            assert!(exts.len() == 2);
            assert!(exts[0].alias == "f1" || exts[0].alias == "f2");
            assert!(exts[0].description == "eeeeee");
            assert!(exts[0].file_name == "f1" || exts[0].file_name == "f2");
            drop(drop_dir);
        });
    }
}
