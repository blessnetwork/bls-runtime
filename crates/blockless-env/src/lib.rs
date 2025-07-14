use blockless_drivers_macro::linker_integration;
use wasmtime::Linker;

linker_integration!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_drivers.witx"],
    target: blockless_drivers::wasi,
    link_method: "add_drivers_to_linker",
});

linker_integration!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_http.witx"],
    target: blockless_drivers::wasi::http,
    link_method: "add_http_to_linker",
});

linker_integration!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_ipfs.witx"],
    target: blockless_drivers::wasi::ipfs,
    link_method: "add_ipfs_to_linker",
});

linker_integration!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_s3.witx"],
    target: blockless_drivers::wasi::s3,
    link_method: "add_s3_to_linker",
});

linker_integration!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_memory.witx"],
    target: blockless_drivers::wasi::memory,
    link_method: "add_memory_to_linker",
});

linker_integration!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_cgi.witx"],
    target: blockless_drivers::wasi::cgi,
    link_method: "add_cgi_to_linker",
});

linker_integration!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_socket.witx"],
    target: blockless_drivers::wasi::socket,
    link_method: "add_socket_to_linker",
});

linker_integration!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_llm.witx"],
    target: blockless_drivers::wasi::llm,
    link_method: "add_llm_to_linker",
});

linker_integration!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_rpc.witx"],
    target: blockless_drivers::wasi::rpc,
    link_method: "add_bless_to_linker",
});
