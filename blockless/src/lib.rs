mod context;
pub mod error;
mod modules;

pub use anyhow::Result as AnyResult;
use anyhow::{Context, bail};
use blockless_drivers::{CdylibDriver, DriverConetxt};
pub use blockless_multiaddr::MultiAddr;
use cap_std::ambient_authority;
use context::BlocklessContext;
pub use error::*;
use log::{debug, error};
use modules::ModuleLinker;
use std::fs::File;
use std::sync::Mutex;
use std::{env, path::Path, sync::Arc};
use wasi_common::sync::WasiCtxBuilder;
use wasi_common::sync::{Dir, TcpListener};
pub use wasi_common::*;
use wasmtime::{
    Config, Engine, Linker, Module, Precompiled, Store, StoreLimits, StoreLimitsBuilder, Trap,
    component::Component,
};
use wasmtime_wasi::IoView;
use wasmtime_wasi::{DirPerms, FilePerms};
use wasmtime_wasi_nn::wit::WasiNnView;
use wasmtime_wasi_threads::WasiThreadsCtx;

// the default wasm entry name.
const ENTRY: &str = "_start";

pub struct ExitStatus {
    pub fuel: Option<u64>,
    pub code: i32,
}

pub enum BlsRunTarget {
    Module(Module),
    Component(Component),
}

impl BlsRunTarget {
    fn unwrap_core(&self) -> &Module {
        match self {
            BlsRunTarget::Module(module) => module,
            BlsRunTarget::Component(_) => panic!("expected a core wasm module, not a component"),
        }
    }

    fn unwrap_component(&self) -> &Component {
        match self {
            BlsRunTarget::Module(_) => panic!("expected a core wasm module, not a module"),
            BlsRunTarget::Component(component) => component,
        }
    }
}

trait BlocklessConfig2Preview1WasiBuilder {
    fn preview1_builder(&self) -> anyhow::Result<WasiCtxBuilder>;
    fn preview2_builder(&self) -> anyhow::Result<wasmtime_wasi::WasiCtxBuilder>;
    fn preview1_set_stdio(&self, builder: &mut WasiCtxBuilder);
    fn preview1_engine_config(&self) -> Config;
    fn store_limits(&self) -> StoreLimits;
}

impl BlocklessConfig2Preview1WasiBuilder for BlocklessConfig {
    /// config to store limit.
    fn store_limits(&self) -> StoreLimits {
        let mut builder = StoreLimitsBuilder::new();
        let store_limited = self.store_limited();
        if let Some(m) = store_limited.max_memory_size {
            builder = builder.memory_size(m);
        }
        if let Some(m) = store_limited.max_instances {
            builder = builder.instances(m);
        }
        if let Some(m) = store_limited.max_table_elements {
            builder = builder.table_elements(m as _);
        }
        if let Some(m) = store_limited.max_tables {
            builder = builder.tables(m as _);
        }
        if let Some(m) = store_limited.max_memories {
            builder = builder.memories(m as _);
        }
        if let Some(m) = store_limited.trap_on_grow_failure {
            builder = builder.trap_on_grow_failure(m);
        }

        builder.build()
    }
    /// set the stdout and stderr for the wasm.
    /// the stdout adn stderr can be setting to file or inherit the stdout and stderr.
    fn preview1_set_stdio(&self, builder: &mut WasiCtxBuilder) {
        let b_conf = self;
        macro_rules! process_output {
            ($out_ref: expr, $out_expr: ident, $stdout: ident, $inherit_stdout: ident) => {
                //$out_ref is b_conf.stdout_ref() or b_conf.stderr_ref()
                match $out_ref {
                    &$out_expr::FileName(ref file_name) => {
                        let mut is_set_fileout = false;
                        if let Some(r) = b_conf.fs_root_path_ref() {
                            let root = Path::new(r);
                            let file_name = root.join(file_name);
                            if let Some(f) = {
                                let mut file_opts = std::fs::File::options();
                                file_opts.create(true);
                                file_opts.append(true);
                                file_opts.write(true);
                                file_opts.open(file_name).ok().map(|file| {
                                    let file = cap_std::fs::File::from_std(file);
                                    let f = wasi_common::sync::file::File::from_cap_std(file);
                                    Box::new(f)
                                })
                            } {
                                is_set_fileout = true;
                                //builder.stdout() or builder.stderr()
                                builder.$stdout(f);
                            }
                        }
                        if !is_set_fileout {
                            //$inherit_stdout is inherit_stdout() or inherit_stderr()
                            builder.$inherit_stdout();
                        }
                    }
                    &$out_expr::Inherit => {
                        builder.$inherit_stdout();
                    }
                    &$out_expr::Null => {}
                }
            };
        }
        process_output!(b_conf.stdout_ref(), Stdout, stdout, inherit_stdout);
        process_output!(b_conf.stderr_ref(), Stderr, stderr, inherit_stderr);

        if let Stdin::Inherit = b_conf.stdio.stdin {
            builder.inherit_stdin();
        }
    }

    /// create the preview1_builder by the configure.
    fn preview1_builder(&self) -> anyhow::Result<WasiCtxBuilder> {
        let b_conf = self;
        let root_dir = b_conf.fs_root_path_ref().and_then(|path| {
            wasi_common::sync::Dir::open_ambient_dir(path, ambient_authority()).ok()
        });
        let mut builder = WasiCtxBuilder::new();
        //stdout file process for setting.
        b_conf.preview1_set_stdio(&mut builder);
        // configure to storeLimit
        let entry_module = b_conf
            .entry_module()
            .context("not found the entry module.")?;
        let mut args = vec![entry_module];
        args.extend_from_slice(&b_conf.stdin_args_ref()[..]);
        builder.args(&args[..])?;
        builder.envs(&b_conf.envs_ref()[..])?;
        let mut max_fd = 3;
        // map host to guest dir in runtime.
        for (host, guest) in b_conf.dirs.iter() {
            let host = Dir::open_ambient_dir(host, ambient_authority())?;
            builder.preopened_dir(host, guest)?;
            max_fd += 1;
        }
        // map root fs
        if let Some(d) = root_dir {
            builder.preopened_dir(d, "/")?;
            max_fd += 1;
        }
        //set the tcp listener.
        for (l, fd) in b_conf.tcp_listens.iter() {
            let fd = if let Some(fd) = fd {
                if *fd < max_fd {
                    bail!("the invalid fd{fd} for listenfd.");
                }
                *fd
            } else {
                let fd = max_fd;
                max_fd += 1;
                fd
            };
            let l = std::net::TcpListener::bind(l)?;
            let l = TcpListener::from_std(l);
            builder.preopened_socket(fd, l)?;
        }
        anyhow::Ok(builder)
    }

    /// convert the blockless configure  to wasmtime configure.
    fn preview1_engine_config(&self) -> Config {
        let mut conf = Config::new();

        if let Some(max) = self.opts.memory_reservation {
            conf.memory_reservation(max);
        }
        if let Some(enable) = self.opts.static_memory_forced {
            conf.memory_may_move(!enable);
        }
        if let Some(size) = self.opts.memory_may_move {
            conf.memory_may_move(size);
        }
        if let Some(size) = self.opts.memory_guard_size {
            conf.memory_guard_size(size);
        }
        if let Some(enable) = self.opts.guard_before_linear_memory {
            conf.guard_before_linear_memory(enable);
        }
        if let Some(enable) = self.opts.memory_init_cow {
            conf.memory_init_cow(enable);
        }
        if let Some(enable) = self.opts.signals_based_traps {
            conf.signals_based_traps(enable);
        }
        if let Some(enable) = self.opts.table_lazy_init {
            conf.table_lazy_init(enable);
        }
        let mem_for_growth = self
            .opts
            .memory_reservation_for_growth
            .or(self.opts.dynamic_memory_reserved_for_growth);
        if let Some(size) = mem_for_growth {
            conf.memory_reservation_for_growth(size);
        }

        if !self.opts.is_empty() {
            if let Some(s) = self.opts.opt_level {
                conf.cranelift_opt_level(s);
            }
            let mut cfg = wasmtime::PoolingAllocationConfig::default();
            if let Some(size) = self.opts.pooling_memory_keep_resident {
                cfg.linear_memory_keep_resident(size);
            }
            if let Some(size) = self.opts.pooling_table_keep_resident {
                cfg.table_keep_resident(size);
            }
            if let Some(limit) = self.opts.pooling_total_core_instances {
                cfg.total_core_instances(limit);
            }
            if let Some(limit) = self.opts.pooling_total_component_instances {
                cfg.total_component_instances(limit);
            }
            if let Some(limit) = self.opts.pooling_total_memories {
                cfg.total_memories(limit);
            }
            if let Some(limit) = self.opts.pooling_total_tables {
                cfg.total_tables(limit);
            }
            if let Some(limit) = self.opts.pooling_table_elements {
                cfg.table_elements(limit as _);
            }
            if let Some(limit) = self.opts.pooling_max_core_instance_size {
                cfg.max_core_instance_size(limit);
            }
            if let Some(limit) = self.opts.pooling_max_memory_size {
                cfg.max_memory_size(limit);
            }
            if let Some(max) = self.opts.pooling_max_component_instance_size {
                cfg.max_component_instance_size(max);
            }
            if let Some(max) = self.opts.pooling_max_core_instances_per_component {
                cfg.max_core_instances_per_component(max);
            }
            if let Some(max) = self.opts.pooling_max_memories_per_component {
                cfg.max_memories_per_component(max);
            }
            if let Some(max) = self.opts.pooling_max_tables_per_component {
                cfg.max_tables_per_component(max);
            }
            if let Some(max) = self.opts.pooling_max_tables_per_module {
                cfg.max_tables_per_module(max);
            }
            if let Some(max) = self.opts.pooling_max_memories_per_module {
                cfg.max_memories_per_module(max);
            }
            if let Some(max) = self.opts.pooling_total_gc_heaps {
                cfg.total_gc_heaps(max);
            }

            conf.allocation_strategy(wasmtime::InstanceAllocationStrategy::Pooling(cfg));
        }
        conf.debug_info(self.get_debug_info());

        if self.get_limited_fuel().is_some() {
            // fuel is enable.
            conf.consume_fuel(true);
        }
        conf.async_support(true);
        if self.feature_thread() {
            conf.wasm_threads(true);
        }
        conf.cache_config_load_default().unwrap();
        conf
    }

    fn preview2_builder(&self) -> anyhow::Result<wasmtime_wasi::WasiCtxBuilder> {
        let mut builder = wasmtime_wasi::WasiCtxBuilder::new();
        for (host_path, guest_path) in self.dirs.iter() {
            builder.preopened_dir(host_path, guest_path, DirPerms::all(), FilePerms::all())?;
        }
        if let Some(root_dir) = self.fs_root_path_ref() {
            builder.preopened_dir(root_dir, "/", DirPerms::all(), FilePerms::all())?;
        }
        Ok(builder)
    }
}

enum BlsLinker {
    Core(wasmtime::Linker<BlocklessContext>),
    Component(wasmtime::component::Linker<BlocklessContext>),
}

impl BlsLinker {
    fn unwrap_core(&mut self) -> &mut wasmtime::Linker<BlocklessContext> {
        match self {
            BlsLinker::Core(linker) => linker,
            BlsLinker::Component(_) => panic!("expected a core linker, not a component linker."),
        }
    }

    #[allow(dead_code)]
    fn unwrap_component(&mut self) -> &mut wasmtime::component::Linker<BlocklessContext> {
        match self {
            BlsLinker::Core(_) => panic!("expected a component linker, not a component linker."),
            BlsLinker::Component(linker) => linker,
        }
    }
}

struct BlocklessRunner(BlocklessConfig);

impl BlocklessRunner {
    /// blockless run method, it execute the wasm program with configure file.
    async fn run(self) -> AnyResult<ExitStatus> {
        let b_conf = &self.0;
        let max_fuel = b_conf.get_limited_fuel();
        // set the drivers root path, if not setting use exe file path.
        let drivers_root_path = b_conf
            .drivers_root_path_ref()
            .map(|p| p.into())
            .unwrap_or_else(|| {
                let mut current_exe_path = env::current_exe().unwrap();
                current_exe_path.pop();
                String::from(current_exe_path.to_str().unwrap())
            });
        DriverConetxt::init_built_in_drivers(drivers_root_path);
        let conf = b_conf.preview1_engine_config();
        let engine = Engine::new(&conf)?;
        let support_thread = b_conf.feature_thread();

        let drivers = b_conf.drivers_ref();
        Self::load_driver(drivers);
        let entry: String = b_conf.entry_ref().into();
        let store_limits = b_conf.store_limits();
        let fule = b_conf.get_limited_fuel();

        let ctx = BlocklessContext {
            store_limits,
            ..Default::default()
        };

        let mut store: Store<BlocklessContext> = Store::new(&engine, ctx);
        store.limiter(|ctx| &mut ctx.store_limits);
        // set the fule in store.
        if let Some(f) = fule {
            store.set_fuel(f).unwrap();
        }
        let (mut linker, run_target, entry) =
            self.module_linker(entry, &engine, &mut store).await?;
        let mut is_component = false;
        if b_conf.nn {
            self.nn_setup(&mut linker, &mut store)?;
        }
        // prepare linker.
        match linker {
            BlsLinker::Core(ref mut linker) => {
                Self::preview1_linker_setup(linker);
                //preview1 setup the permissions with options
                store.data_mut().set_permisions(&b_conf.permissions_config);
            }
            BlsLinker::Component(ref mut linker) => {
                is_component = true;
                wasmtime_wasi::add_to_linker_async(linker)?;
                self.preview2_setup(store.data_mut())?;
            }
        }
        // support thread.
        if support_thread {
            Self::preview1_setup_thread_support(
                linker.unwrap_core(),
                &mut store,
                run_target.unwrap_core(),
            );
        }

        let result =
            Self::load_main_module(b_conf, &mut store, &mut linker, &run_target, &entry).await;
        let exit_code = match result {
            Err(ref t) => {
                Self::error_process(is_component, t, || store.get_fuel().unwrap(), max_fuel)
            }
            Ok(_) => {
                debug!("program exit normal.");
                0
            }
        };
        Ok(ExitStatus {
            fuel: store.get_fuel().ok(),
            code: exit_code,
        })
    }

    fn preview1_setup(&self, ctx: &mut BlocklessContext) -> AnyResult<()> {
        let mut builder = self.0.preview1_builder()?;
        let mut preview1_ctx = builder.build();
        preview1_ctx.set_blockless_config(Some(self.0.clone()));
        ctx.preview1_ctx = Some(preview1_ctx);
        Ok(())
    }

    fn preview2_setup(&self, ctx: &mut BlocklessContext) -> AnyResult<()> {
        let mut builder = self.0.preview2_builder()?;
        builder.inherit_stdio().args(&self.0.stdin_args);
        builder.envs(&self.0.envs);
        let preview2_ctx = builder.build_p1();
        ctx.preview2_ctx = Some(Arc::new(Mutex::new(preview2_ctx)));
        Ok(())
    }

    fn collect_preloaded_nn_graphs(
        &self,
    ) -> AnyResult<(Vec<wasmtime_wasi_nn::Backend>, wasmtime_wasi_nn::Registry)> {
        let graphs = self
            .0
            .nn_graph
            .iter()
            .map(|g| (g.format.clone(), g.dir.clone()))
            .collect::<Vec<_>>();
        wasmtime_wasi_nn::preload(&graphs)
    }

    fn write_core_dump(
        store: &mut Store<BlocklessContext>,
        err: &anyhow::Error,
        name: &str,
        path: &str,
    ) -> AnyResult<()> {
        use std::fs::File;
        use std::io::Write;

        let core_dump = err
            .downcast_ref::<wasmtime::WasmCoreDump>()
            .expect("should have been configured to capture core dumps");

        let core_dump = core_dump.serialize(store, name);

        let mut core_dump_file =
            File::create(path).context(format!("failed to create file at `{path}`"))?;
        core_dump_file
            .write_all(&core_dump)
            .with_context(|| format!("failed to write core dump file at `{path}`"))?;
        Ok(())
    }

    async fn load_main_module(
        cfg: &BlocklessConfig,
        store: &mut Store<BlocklessContext>,
        linker: &mut BlsLinker,
        module: &BlsRunTarget,
        entry: &str,
    ) -> AnyResult<()> {
        // The main module might be allowed to have unknown imports, which
        // should be defined as traps:
        if cfg.unknown_imports_trap {
            match linker {
                BlsLinker::Core(linker) => {
                    linker.define_unknown_imports_as_traps(module.unwrap_core())?;
                }
                BlsLinker::Component(linker) => {
                    linker.define_unknown_imports_as_traps(module.unwrap_component())?;
                }
            }
        }

        let result = match linker {
            BlsLinker::Core(linker) => {
                let module = module.unwrap_core();
                let instance = linker.instantiate_async(&mut *store, module).await?;

                // If `_initialize` is present, meaning a reactor, then invoke the function.
                if let Some(func) = instance.get_func(&mut *store, "_initialize") {
                    let init = func.typed::<(), ()>(&store)?;
                    init.call_async(&mut *store, ()).await?;
                }
                // Look for the specific function provided or otherwise look for
                // "" or "_start" exports to run as a "main" function.
                let func = match cfg.version {
                    BlocklessConfigVersion::Version0 => instance
                        .get_typed_func(&mut *store, entry)
                        .or_else(|_| instance.get_typed_func::<(), ()>(&mut *store, ""))
                        .or_else(|_| instance.get_typed_func::<(), ()>(&mut *store, ENTRY))?,
                    BlocklessConfigVersion::Version1 => {
                        instance.get_typed_func::<(), ()>(&mut *store, entry)?
                    }
                };
                // if thread multi thread use sync model.
                // The multi-thread model is used for the cpu intensive program.
                func.call_async(&mut *store, ()).await
            }
            BlsLinker::Component(linker) => {
                let component = module.unwrap_component();
                let command = wasmtime_wasi::bindings::Command::instantiate_async(
                    &mut *store,
                    component,
                    linker,
                )
                .await?;
                let result = command
                    .wasi_cli_run()
                    .call_run(&mut *store)
                    .await
                    .context("failed to invoke `run` function")
                    .map_err(|e| Self::handle_core_dump(cfg, &mut *store, e));
                // Translate the `Result<(),()>` produced by wasm into a feigned
                // explicit exit here with status 1 if `Err(())` is returned.
                result.and_then(|wasm_result| match wasm_result {
                    Ok(()) => Ok(()),
                    Err(()) => Err(wasmtime_wasi::I32Exit(1).into()),
                })
            }
        };
        result
    }

    fn nn_setup(
        &self,
        linker: &mut BlsLinker,
        store: &mut Store<BlocklessContext>,
    ) -> AnyResult<()> {
        let (backends, registry) = self.collect_preloaded_nn_graphs()?;
        match linker {
            BlsLinker::Core(linker) => {
                wasmtime_wasi_nn::witx::add_to_linker(linker, |host| {
                    Arc::get_mut(host.wasi_nn_witx.as_mut().unwrap())
                        .expect("wasi-nn is not implemented with multi-threading support")
                })?;
                store.data_mut().wasi_nn_witx = Some(Arc::new(
                    wasmtime_wasi_nn::witx::WasiNnCtx::new(backends, registry),
                ));
            }
            BlsLinker::Component(linker) => {
                wasmtime_wasi_nn::wit::add_to_linker(linker, |h: &mut BlocklessContext| {
                    let preview2_ctx = h.preview2_ctx.as_mut().expect("wasip2 is not configured");
                    let preview2_ctx = Arc::get_mut(preview2_ctx)
                        .expect("wasmtime_wasi is not compatible with threads")
                        .get_mut()
                        .unwrap();
                    let nn_ctx = Arc::get_mut(h.wasi_nn_wit.as_mut().unwrap())
                        .expect("wasi-nn is not implemented with multi-threading support");
                    WasiNnView::new(preview2_ctx.table(), nn_ctx)
                })?;
                store.data_mut().wasi_nn_wit = Some(Arc::new(
                    wasmtime_wasi_nn::wit::WasiNnCtx::new(backends, registry),
                ));
            }
        }
        Ok(())
    }

    fn handle_core_dump(
        cfg: &BlocklessConfig,
        store: &mut Store<BlocklessContext>,
        err: anyhow::Error,
    ) -> anyhow::Error {
        let coredump_path = match &cfg.coredump {
            Some(path) => path,
            None => return err,
        };
        if !err.is::<wasmtime::Trap>() {
            return err;
        }
        let source_name = cfg.modules[0].file.as_str();

        if let Err(coredump_err) = Self::write_core_dump(store, &err, source_name, coredump_path) {
            eprintln!("warning: coredump failed to generate: {coredump_err}");
            err
        } else {
            err.context(format!("core dumped at {coredump_path}"))
        }
    }

    pub fn load_module<T: AsRef<Path>>(engine: &Engine, path: T) -> AnyResult<BlsRunTarget> {
        let path: &Path = match path.as_ref().to_str() {
            #[cfg(unix)]
            Some("-") => "/dev/stdin".as_ref(),
            _ => path.as_ref(),
        };
        let file = File::open(path)?;
        match wasmtime::_internal::MmapVec::from_file(file) {
            Ok(map) => Self::load_module_contents(
                engine,
                path,
                &map,
                || unsafe { Module::deserialize_file(engine, path) },
                || unsafe { Component::deserialize_file(engine, path) },
            ),
            Err(_) => {
                let bytes = std::fs::read(path)
                    .with_context(|| format!("failed to read file: {}", path.display()))?;
                Self::load_module_contents(
                    engine,
                    path,
                    &bytes,
                    || unsafe { Module::deserialize(engine, &bytes) },
                    || unsafe { Component::deserialize(engine, &bytes) },
                )
            }
        }
    }

    pub fn load_module_contents(
        engine: &Engine,
        path: &Path,
        bytes: &[u8],
        deserialize_module: impl FnOnce() -> AnyResult<Module>,
        deserialize_component: impl FnOnce() -> AnyResult<Component>,
    ) -> AnyResult<BlsRunTarget> {
        Ok(match engine.detect_precompiled(bytes) {
            Some(Precompiled::Module) => BlsRunTarget::Module(deserialize_module()?),
            Some(Precompiled::Component) => BlsRunTarget::Component(deserialize_component()?),
            None => {
                let mut code = wasmtime::CodeBuilder::new(engine);
                code.wasm_binary_or_text(bytes, Some(path))?;
                match code.hint() {
                    Some(wasmtime::CodeHint::Component) => {
                        BlsRunTarget::Component(code.compile_component()?)
                    }
                    Some(wasmtime::CodeHint::Module) | None => {
                        BlsRunTarget::Module(code.compile_module()?)
                    }
                }
            }
        })
    }

    fn preview1_linker_setup(linker: &mut Linker<BlocklessContext>) {
        // define the macro of extends.
        macro_rules! add_to_linker {
            ($method:expr) => {
                $method(linker, |s| s.preview1_ctx.as_mut().unwrap()).unwrap()
            };
        }
        add_to_linker!(blockless_env::add_drivers_to_linker);
        add_to_linker!(blockless_env::add_http_to_linker);
        add_to_linker!(blockless_env::add_llm_to_linker);
        add_to_linker!(blockless_env::add_ipfs_to_linker);
        add_to_linker!(blockless_env::add_s3_to_linker);
        add_to_linker!(blockless_env::add_memory_to_linker);
        add_to_linker!(blockless_env::add_cgi_to_linker);
        add_to_linker!(blockless_env::add_socket_to_linker);
        add_to_linker!(blockless_env::add_bless_to_linker);
        wasi_common::sync::add_to_linker(linker, |host| host.preview1_ctx.as_mut().unwrap())
            .unwrap();
    }

    fn preview1_setup_thread_support(
        linker: &mut Linker<BlocklessContext>,
        store: &mut Store<BlocklessContext>,
        module: &Module,
    ) {
        wasmtime_wasi_threads::add_to_linker(linker, store, module, |ctx| {
            ctx.wasi_threads.as_ref().unwrap()
        })
        .unwrap();
        store.data_mut().wasi_threads = Some(Arc::new(
            WasiThreadsCtx::new(module.clone(), Arc::new(linker.clone()))
                .expect("wasi thread ctx new fail."),
        ));
    }

    async fn module_linker(
        &self,
        mut entry: String,
        engine: &Engine,
        store: &'_ mut Store<BlocklessContext>,
    ) -> anyhow::Result<(BlsLinker, BlsRunTarget, String)> {
        let version = self.0.version();
        match version {
            // this is older configure for bls-runtime, this only run single wasm.
            BlocklessConfigVersion::Version0 => {
                let module = Self::load_module(engine, &entry)?;
                let linker = match module {
                    BlsRunTarget::Module(_) => {
                        self.preview1_setup(store.data_mut())?;
                        BlsLinker::Core(wasmtime::Linker::new(engine))
                    }
                    BlsRunTarget::Component(_) => {
                        BlsLinker::Component(wasmtime::component::Linker::new(engine))
                    }
                };
                Ok((linker, module, ENTRY.to_string()))
            }
            BlocklessConfigVersion::Version1 => {
                if entry.is_empty() {
                    entry = ENTRY.to_string();
                }
                // must setup before link_modules.
                self.preview1_setup(store.data_mut())?;
                let mut linker = wasmtime::Linker::new(engine);
                let mut module_linker = ModuleLinker::new(&mut linker, store);
                let module = module_linker.link_modules().await.context("")?;
                Ok((BlsLinker::Core(linker), BlsRunTarget::Module(module), entry))
            }
        }
    }

    fn load_driver(cfs: &[DriverConfig]) {
        cfs.iter().for_each(|cfg| {
            let drv = CdylibDriver::load(cfg.path(), cfg.schema()).unwrap();
            DriverConetxt::insert_driver(drv);
        });
    }

    /// the error code process.
    fn error_process<F>(
        is_component: bool,
        e: &anyhow::Error,
        used_fuel: F,
        max_fuel: Option<u64>,
    ) -> i32
    where
        F: FnOnce() -> u64,
    {
        if is_component {
            if let Some(exit) = e.downcast_ref::<wasmtime_wasi::I32Exit>() {
                std::process::exit(exit.0);
            }
        } else if let Some(exit) = e.downcast_ref::<wasi_common::I32Exit>() {
            std::process::exit(exit.0);
        }
        let trap_code_2_exit_code = |trap_code: &Trap| -> Option<i32> {
            match *trap_code {
                Trap::OutOfFuel => Some(1),
                Trap::StackOverflow => Some(2),
                Trap::MemoryOutOfBounds => Some(3),
                Trap::HeapMisaligned => Some(4),
                Trap::TableOutOfBounds => Some(5),
                Trap::IndirectCallToNull => Some(6),
                Trap::BadSignature => Some(7),
                Trap::IntegerOverflow => Some(8),
                Trap::IntegerDivisionByZero => Some(9),
                Trap::BadConversionToInteger => Some(10),
                Trap::UnreachableCodeReached => Some(11),
                Trap::Interrupt => Some(12),
                Trap::AlwaysTrapAdapter => Some(13),
                _ => None,
            }
        };
        let trap = e.downcast_ref::<Trap>();
        let rs = trap
            .and_then(trap_code_2_exit_code)
            .or_else(|| {
                if e.to_string().starts_with("memory minimum size of") {
                    Some(16)
                } else {
                    None
                }
            })
            .unwrap_or(-1);
        match trap {
            Some(Trap::OutOfFuel) => {
                let used_fuel = used_fuel();
                let max_fuel = max_fuel.unwrap_or_default();
                error!(
                    "All fuel is consumed, the app exited, fuel consumed {}, Max Fuel is {}.",
                    used_fuel, max_fuel
                );
            }
            _ => error!("error: {}", e),
        };
        rs
    }
}

pub async fn blockless_run(b_conf: BlocklessConfig) -> anyhow::Result<ExitStatus> {
    BlocklessRunner(b_conf).run().await
}

#[cfg(test)]
mod test {
    #[allow(unused_imports)]
    use super::*;

    //inner test
    #[test]
    fn test_exit_code() {
        let err = Trap::OutOfFuel.into();
        let rs = BlocklessRunner::error_process(false, &err, || 20u64, Some(30));
        assert_eq!(rs, 1);
    }
}
