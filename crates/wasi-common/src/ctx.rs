use crate::clocks::WasiClocks;
use crate::dir::{DirEntry, WasiDir};
use crate::file::{FileAccessMode, FileEntry, WasiFile};
use crate::sched::WasiSched;
use crate::string_array::StringArray;
use crate::table::Table;
use crate::{BlocklessConfig, BlsRuntimePermissionsContainer, PermissionsConfig};
use crate::{Error, StringArrayError};
use bls_permissions::Url;
use cap_rand::RngCore;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// An `Arc`-wrapper around the wasi-common context to allow mutable access to
/// the file descriptor table. This wrapper is only necessary due to the
/// signature of `fd_fdstat_set_flags`; if that changes, there are a variety of
/// improvements that can be made (TODO:
/// <https://github.com/bytecodealliance/wasmtime/issues/5643)>.
#[derive(Clone)]
pub struct WasiCtx(Arc<WasiCtxInner>);

pub struct WasiCtxInner {
    pub args: StringArray,
    pub env: StringArray,
    // TODO: this mutex should not be necessary, it forces threads to serialize
    // their access to randomness unnecessarily
    // (https://github.com/bytecodealliance/wasmtime/issues/5660).
    pub random: Mutex<Box<dyn RngCore + Send + Sync>>,
    pub clocks: WasiClocks,
    pub sched: Box<dyn WasiSched>,
    pub table: Table,
    pub perms_container: BlsRuntimePermissionsContainer,
    pub blockless_config: Mutex<Option<BlocklessConfig>>,
}

impl WasiCtx {
    pub fn new(
        random: Box<dyn RngCore + Send + Sync>,
        clocks: WasiClocks,
        sched: Box<dyn WasiSched>,
        table: Table,
        cwd: Option<&str>,
    ) -> Self {
        let s = WasiCtx(Arc::new(WasiCtxInner {
            args: StringArray::new(),
            env: StringArray::new(),
            random: Mutex::new(random),
            blockless_config: Mutex::new(None),
            perms_container: BlsRuntimePermissionsContainer::new_with_env_cwd(cwd),
            clocks,
            sched,
            table,
        }));
        s.set_stdin(Box::new(crate::pipe::ReadPipe::new(std::io::empty())));
        s.set_stdout(Box::new(crate::pipe::WritePipe::new(std::io::sink())));
        s.set_stderr(Box::new(crate::pipe::WritePipe::new(std::io::sink())));
        s
    }

    pub fn set_blockless_config(&mut self, c: Option<BlocklessConfig>) {
        let mut lock = self.0.blockless_config.lock().unwrap();
        c.map(|c| lock.replace(c));
    }

    pub fn config_drivers_root_path_ref(&mut self) -> Option<String> {
        let lock = self.0.blockless_config.lock().unwrap();
        lock.as_ref()
            .and_then(|l| l.drivers_root_path_ref().map(String::from))
    }

    pub fn config_stdin_ref(&mut self) -> Option<String> {
        let lock = self.0.blockless_config.lock().unwrap();
        lock.as_ref()
            .and_then(|l| l.fix_stdin_ref().map(String::from))
    }

    pub fn check_url_permissions(&self, host: &Url, api_name: &str) -> bool {
        match self.perms_container.check_net_url(host, api_name) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub fn resource_permission(&self, resource: &str) -> bool {
        match self.blockless_config.lock().unwrap().deref() {
            Some(c) => c.resource_permission(resource),
            None => false,
        }
    }

    pub fn set_permissions_config(
        &mut self,
        config: &PermissionsConfig,
    ) -> Result<(), anyhow::Error> {
        self.perms_container.set_permissions_config(config)
    }

    pub fn insert_file(&self, fd: u32, file: Box<dyn WasiFile>, access_mode: FileAccessMode) {
        self.table()
            .insert_at(fd, Arc::new(FileEntry::new(file, access_mode)));
    }

    pub fn push_file(
        &self,
        file: Box<dyn WasiFile>,
        access_mode: FileAccessMode,
    ) -> Result<u32, Error> {
        self.table()
            .push(Arc::new(FileEntry::new(file, access_mode)))
    }

    pub fn insert_dir(&self, fd: u32, dir: Box<dyn WasiDir>, path: PathBuf) {
        self.table()
            .insert_at(fd, Arc::new(DirEntry::new(Some(path), dir)));
    }

    pub fn push_dir(&self, dir: Box<dyn WasiDir>, path: PathBuf) -> Result<u32, Error> {
        self.table().push(Arc::new(DirEntry::new(Some(path), dir)))
    }

    pub fn table(&self) -> &Table {
        &self.table
    }

    pub fn table_mut(&mut self) -> Option<&mut Table> {
        Arc::get_mut(&mut self.0).map(|c| &mut c.table)
    }

    pub fn push_arg(&mut self, arg: &str) -> Result<(), StringArrayError> {
        let s = Arc::get_mut(&mut self.0).expect(
            "`push_arg` should only be used during initialization before the context is cloned",
        );
        s.args.push(arg.to_owned())
    }

    pub fn push_env(&mut self, var: &str, value: &str) -> Result<(), StringArrayError> {
        let s = Arc::get_mut(&mut self.0).expect(
            "`push_env` should only be used during initialization before the context is cloned",
        );
        s.env.push(format!("{var}={value}"))?;
        Ok(())
    }

    pub fn set_stdin(&self, f: Box<dyn WasiFile>) {
        self.insert_file(0, f, FileAccessMode::READ);
    }

    pub fn set_stdout(&self, f: Box<dyn WasiFile>) {
        self.insert_file(1, f, FileAccessMode::WRITE);
    }

    pub fn set_stderr(&self, f: Box<dyn WasiFile>) {
        self.insert_file(2, f, FileAccessMode::WRITE);
    }

    pub fn push_preopened_dir(
        &self,
        dir: Box<dyn WasiDir>,
        path: impl AsRef<Path>,
    ) -> Result<(), Error> {
        self.table()
            .push(Arc::new(DirEntry::new(Some(path.as_ref().to_owned()), dir)))?;
        Ok(())
    }
}

impl Deref for WasiCtx {
    type Target = WasiCtxInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
