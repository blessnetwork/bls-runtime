use std::sync::{Arc, Mutex};

use wasmtime_wasi_threads::WasiThreadsCtx;

#[derive(Clone)]
pub(crate) struct BlocklessContext {
    pub(crate) preview1_ctx: Option<wasi_common::WasiCtx>,

    pub(crate) preview2_ctx: Option<Arc<Mutex<wasmtime_wasi::WasiP1Ctx>>>,

    pub(crate) wasi_threads: Option<Arc<WasiThreadsCtx<BlocklessContext>>>,
}

impl Default for BlocklessContext {
    fn default() -> Self {
        Self {
            preview1_ctx: None,
            preview2_ctx: None,
            wasi_threads: None,
        }
    }
}

impl BlocklessContext {
    pub(crate) fn preview2_ctx(&mut self) -> &mut wasmtime_wasi::WasiP1Ctx {
        let ctx = self.preview2_ctx.as_mut().unwrap();
        Arc::get_mut(ctx)
            .expect("wasmtime_wasi was not compatiable threads")
            .get_mut()
            .unwrap()
    }
}

impl wasmtime_wasi::WasiView for BlocklessContext {
    fn table(&mut self) -> &mut wasmtime::component::ResourceTable {
        self.preview2_ctx().table()
    }

    fn ctx(&mut self) -> &mut wasmtime_wasi::WasiCtx {
        self.preview2_ctx().ctx()
    }
}
