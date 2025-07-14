#![allow(static_mut_refs)]
mod cdylib_driver;
pub mod cgi_driver;
pub mod error;
pub mod http_driver;
pub mod ipfs_driver;
pub mod llm_driver;
pub mod memory_driver;
pub mod read_ext;
pub mod s3_driver;
pub mod tcp_driver;
pub mod wasi;
pub mod handlers;
use blockless_multiaddr as multiaddr;
pub use cdylib_driver::CdylibDriver;
pub use error::*;
#[cfg(not(feature = "builtin_http"))]
use http_driver::init_http_driver;

use lazy_static::*;
use log::error;
use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use tcp_driver::TcpDriver;
use wasi_common::WasiFile;

type OpenFuture = Pin<Box<dyn Future<Output = Result<Box<dyn WasiFile>, ErrorKind>> + Send>>;

pub trait Driver {
    fn name(&self) -> &str;

    fn open(&self, uri: &str, opts: &str) -> OpenFuture;
}

lazy_static! {
    pub static ref DRIVERS: Mutex<DriverContextImpl> = Mutex::new(DriverContextImpl::new());
}

pub struct DriverContextImpl {
    drivers: HashMap<String, Arc<dyn Driver + Sync + Send>>,
}

impl DriverContextImpl {
    fn new() -> Self {
        DriverContextImpl {
            drivers: HashMap::new(),
        }
    }

    fn insert_driver<T>(&mut self, driver: T)
    where
        T: Driver + Send + Sync + 'static,
    {
        let key = driver.name().to_lowercase();
        self.drivers.insert(key, Arc::new(driver));
    }

    fn find_driver(&self, uri: &str) -> Option<Arc<dyn Driver + Sync + Send>> {
        let addr = match multiaddr::parse(uri.as_bytes()) {
            Err(e) => {
                error!("error parse:{:?}", e);
                return None;
            }
            Ok(addr) => addr,
        };
        let schema = match addr.schema() {
            Err(e) => {
                error!("get schema error:{:?}", e);
                return None;
            }
            Ok(s) => s.to_lowercase(),
        };
        self.drivers.get(&schema).cloned()
    }
}

pub struct DriverConetxt;

impl DriverConetxt {
    pub fn find_driver(uri: &str) -> Option<Arc<dyn Driver + Sync + Send>> {
        let drv = DRIVERS.lock().unwrap();
        drv.find_driver(uri)
    }

    pub fn insert_driver<T: Driver + Sync + Send + 'static>(driver: T) {
        let mut drv = DRIVERS.lock().unwrap();
        drv.insert_driver(driver);
    }

    #[warn(unused_variables)]
    pub fn init_built_in_drivers(_path: impl AsRef<Path>) {
        #[cfg(not(feature = "builtin_http"))]
        let tcp_driver_path = _path.as_ref().join("http_driver.so");
        #[cfg(not(feature = "builtin_http"))]
        if tcp_driver_path.exists() {
            init_http_driver(tcp_driver_path.as_os_str()).unwrap();
        }
        Self::insert_driver(TcpDriver {});
    }
}
