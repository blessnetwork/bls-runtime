# Runtime Environment

![](blockless.png)

## Features

The runtime is built on WebAssembly (Wasm) technology and therefore has the following features:

- Speed: It is built with an optimizing code generator to generate high-quality machine code quickly. The runtime is also optimized for efficient instantiation, low-overhead transitions between the embedder and Wasm, and scalability of concurrent instances.

- Compatibility: It supports running standard bytecode programs compiled from various programming languages such as C/C++, Rust, Swift, AssemblyScript, or Kotlin. It also supports mixing these languages (e.g. using Rust to implement a JavaScript API).

- Customizability: The runtime provides a configurable file to offer various options such as additional restrictions on WebAssembly beyond its basic guarantees, including CPU and memory consumption.

## Architecture

```mermaid
graph TD
    subgraph "Configuration Layer"
        CLI["bls-runtime (CLI/Config Loader)"]
        LOGGING["Logging (runtime_logger)"]
    end

    subgraph "Core Engine"
        CORE["blockless (Core Runtime Engine)"]
    end

    subgraph "Driver Modules"
        DRIVER_ROOT["blockless-drivers"]
        CDYLIB["cdylib Driver"]
        CGI["CGI Driver"]
        HTTP["HTTP Driver"]
        IPFS["IPFS Driver"]
        LLM["LLM Driver"]
        MEMORY["Memory Driver"]
        S3["S3 Driver"]
        TCP["TCP Driver"]
        WASI["WASI Support Files"]
        WITX["Interface Definitions (Witx)"]
    end

    subgraph "Auxiliary Libraries"
        ENV["blockless-env"]
        MULTI["blockless-multiaddr"]
        WASI_COMMON["wasi-common"]
    end

    subgraph "Supported Languages"
        RUST["Rust Modules"]
        GO["Go Modules"]
        AS["AssemblyScript Modules"]
    end

    CLI -->|"loadsConfig"| CORE
    CORE -->|"passesModules"| DRIVER_ROOT
    CORE -->|"uses"| ENV
    CORE -->|"uses"| MULTI
    CORE -->|"uses"| WASI_COMMON
    CORE -->|"logsTo"| LOGGING
    RUST -->|"WasmCompiled"| CORE
    GO -->|"WasmCompiled"| CORE
    AS -->|"WasmCompiled"| CORE

    DRIVER_ROOT -->|"contains"| CDYLIB
    DRIVER_ROOT -->|"contains"| CGI
    DRIVER_ROOT -->|"contains"| HTTP
    DRIVER_ROOT -->|"contains"| IPFS
    DRIVER_ROOT -->|"contains"| LLM
    DRIVER_ROOT -->|"contains"| MEMORY
    DRIVER_ROOT -->|"contains"| S3
    DRIVER_ROOT -->|"contains"| TCP
    DRIVER_ROOT -->|"contains"| WASI
    DRIVER_ROOT -->|"contains"| WITX

    class CLI,CORE,LOGGING core
    class DRIVER_ROOT,CDYLIB,CGI,HTTP,IPFS,LLM,MEMORY,S3,TCP,WASI,WITX driver
    class ENV,MULTI,WASI_COMMON aux
    class RUST,GO,AS language

    click CORE "https://github.com/blessnetwork/bls-runtime/tree/main/blockless/"
    click CLI "https://github.com/blessnetwork/bls-runtime/tree/main/bls-runtime/"
    click DRIVER_ROOT "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-drivers/"
    click CDYLIB "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-drivers/src/cdylib_driver/"
    click CGI "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-drivers/src/cgi_driver/"
    click HTTP "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-drivers/src/http_driver/"
    click IPFS "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-drivers/src/ipfs_driver/"
    click LLM "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-drivers/src/llm_driver/"
    click MEMORY "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-drivers/src/memory_driver/"
    click S3 "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-drivers/src/s3_driver/"
    click TCP "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-drivers/src/tcp_driver/"
    click WASI "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-drivers/src/wasi/"
    click WITX "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-drivers/witx/"
    click ENV "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-env/"
    click MULTI "https://github.com/blessnetwork/bls-runtime/tree/main/crates/blockless-multiaddr/"
    click WASI_COMMON "https://github.com/blessnetwork/bls-runtime/tree/main/crates/wasi-common/"
    click GO "https://github.com/blessnetwork/bls-runtime/tree/main/examples/golang/"

    classDef core fill:#87CEFA,stroke:#000,stroke-width:2px;
    classDef driver fill:#F4A460,stroke:#000,stroke-width:2px;
    classDef aux fill:#DA70D6,stroke:#000,stroke-width:2px;
    classDef language fill:#98FB98,stroke:#000,stroke-width:2px;
```

## Building the Project
1. Install Rust by visiting the website 'https://rustup.rs/'

2. Run the following command to build the project:
```
$ cargo build
```

## Supported Languages

Blockless supports a variety of programming languages including:

- [Go] - Tiny Go support.
- [Rust] - Blockless crate.
- [Typescript] - AssemblyScript Support.

[Go]: https://github.com/txlabs/blockless-sdk-golang
[Rust]: https://github.com/txlabs/blockless-sdk-rust
[Typescript]: https://github.com/txlabs/blockless-sdk-assemblyscript


## The example of configure file 

```jsonp
{
    "fs_root_path": "/Users/join/Downloads", 
    "drivers_root_path": "/Users/join/Downloads", 
    "runtime_logger": "runtime.log", 
    "limited_fuel": 200000000,
    "limited_memory": 30,
    "entry": "main",
    "modules": [
        {
            "file": "/Users/join/Downloads/test1.wasm",
            "name": "linking2",
            "type": "module",
            "md5": "d41d8cd98f00b204e9800998ecf8427e"
        }
    ],
    "permissions": [
        "http://httpbin.org/anything",
        "file://a.go"
    ],
    "optimize": {
        //Optimization level of generated code (n:None, s:Speed, ss:SpeedAndSize; default: ss)
        "opt_level": "ss",
        //The maximum number of WebAssembly memories which can be created with the pooling allocator.
        "pooling_total_memories": 1024000,
        //The maximum number of WebAssembly tables which can be created with the pooling allocator.
        "pooling_total_tables": 20,
        // Whether to initialize tables lazily, so that instantiation is fast but indirect calls are a little slower. If no, tables are initialized eagerly from any active element segments that apply to them during instantiation. (default: yes)
        "table_lazy_init": false,
        // Enable the pooling allocator, in place of the on-demand allocator.
        "pooling_allocator": true, 
        ...
    }
}
```

- `fs_root_path`: The root file system path of the app. When the app is opened, it will use this file system as its "/".

- `limited_fuel`: The limit of instructions the app can execute. In the example, the limit is 200000000. If the app exceeds the limit, it will be interrupted and the following message will be displayed:

```log
[2022-06-07T22:12:47Z ERROR blockless] All fuel is consumed, the app exited, fuel consumed 2013, Max Fuel is 2000.
```

- `limited_memory`: The maximum size of memory that the app can use. In the example, the maximum is 20 pages, where each page is 64k. So, the app can only use 20 * 64k of physical memory.

- `entry`: The entry is the function name. Please refer to the app example for more information.

- `permissions`: a list of resources that the app is allowed to access. If the app tries to access a resource that is not in this list, it will receive a "Permission Deny" error. If the app panics, the log will show the following message:

- `modules`: is the app wasm files. the wasm files have 2 types defined by `type` node, `module` and `entry`. `module` is lib in the app, `entry` is the entry wasm, normally the entry wasm contain the entry function.
    - `type`: he wasm files have 2 types defined by `type` node.
    - `file`: the wasm file.
    - `name`: name is used for define the linker name, the app can be use the name for the caller.
    - `md5`: the checksum of the file.

```log
panic: Permission deny
[2022-06-09T02:12:39Z ERROR blockless] Fuel 137607:200000000. wasm trap: wasm `unreachable` instruction executed
```

- `runtime_logger`: Specifies the path to the log file for the runtime environment. In the example above, all log output will be written to the file /path/to/log/file.log.

- `drivers_root_path`: Specifies the root path for the drivers used by the runtime environment. In the example above, the drivers will be stored in the directory /path/to/drivers.

for the file permission the url is start with "file://", if you use "file:///", should not work.

## Using the runtime from the command line

The runtime requires an input from stdin and also accepts environment variables passed as a list separated by ; through the BLS_LIST_VARS variable. Here's an example of how to run the app:

```bash
$ "echo "FOO" | env THIS_IS_MY_VAR=FOO BLS_LIST_VARS=THIS_IS_MY_VAR ~/.bls/runtime/blockless-cli ./build/manifest.json"
```

## Exit codes

|code|description|
|----|-------------------|
|Exit Code 0|Success|
|Exit Code 1|The flue used out|
|Exit Code 2|call stack exhausted|
|Exit Code 3|out of bounds memory access|
|Exit Code 4|misaligned memory access|
|Exit Code 5|undefined element: out of bounds table access|
|Exit Code 6|uninitialized element|
|Exit Code 7|indirect call type mismatch|
|Exit Code 8|integer overflow|
|Exit Code 9|integer divide by zero|
|Exit Code 10|invalid conversion to integer|
|Exit Code 11|wasm `unreachable` instruction executed|
|Exit Code 12|interrupt|
|Exit Code 13|degenerate component adapter called|
|Exit Code 15|the app timeout|
|Exit Code 128|The configure error|
|Exit Code 255|Unknown error|

