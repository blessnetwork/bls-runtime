use std::sync::Arc;

use wasi_common::{
    file::{FileAccessMode, FileEntry},
    sync::net::Socket,
    WasiCtx, WasiFile,
};

use crate::BlocklessSocketErrorKind;
use log::error;
use std::net::{TcpListener, TcpStream};
use wiggle::{GuestMemory, GuestPtr};

wiggle::from_witx!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_socket.witx"],
    errors: { socket_error => BlocklessSocketErrorKind },
    async: *,
    wasmtime: false,
});

impl types::UserErrorConversion for WasiCtx {
    fn socket_error_from_blockless_socket_error_kind(
        &mut self,
        e: self::BlocklessSocketErrorKind,
    ) -> wiggle::anyhow::Result<types::SocketError> {
        Ok(e.into())
    }
}

impl wiggle::GuestErrorType for types::SocketError {
    fn success() -> Self {
        Self::Success
    }
}

impl From<BlocklessSocketErrorKind> for types::SocketError {
    fn from(e: BlocklessSocketErrorKind) -> types::SocketError {
        use types::SocketError;
        match e {
            BlocklessSocketErrorKind::AddressInUse => SocketError::AddressInUse,
            BlocklessSocketErrorKind::ConnectRefused => SocketError::ConnectionRefused,
            BlocklessSocketErrorKind::ConnectionReset => SocketError::ConnectionReset,
            BlocklessSocketErrorKind::ParameterError => SocketError::ParameterError,
        }
    }
}

async fn tcp_connect(addr: &str) -> Result<Box<dyn WasiFile>, BlocklessSocketErrorKind> {
    let stream = match TcpStream::connect(addr) {
        Ok(s) => s,
        Err(e) => {
            error!("error connect in driver {}: {}", addr, e);
            return Err(BlocklessSocketErrorKind::ConnectRefused);
        }
    };
    let stream = cap_std::net::TcpStream::from_std(stream);
    let socket: Socket = Socket::from(stream);
    let wasi_file: Box<dyn WasiFile> = Box::<dyn WasiFile>::from(socket);
    Ok(wasi_file)
}

async fn tcp_bind(addr: &str) -> Result<Box<dyn WasiFile>, BlocklessSocketErrorKind> {
    let listener = match TcpListener::bind(addr) {
        Ok(s) => s,
        Err(e) => {
            error!("error connect in driver {}: {}", addr, e);
            return Err(BlocklessSocketErrorKind::ConnectRefused);
        }
    };
    let listener = cap_std::net::TcpListener::from_std(listener);
    let socket: Socket = Socket::from(listener);
    let wasi_file: Box<dyn WasiFile> = Box::<dyn WasiFile>::from(socket);
    Ok(wasi_file)
}

#[wiggle::async_trait]
impl blockless_socket::BlocklessSocket for WasiCtx {
    async fn create_tcp_bind_socket(
        &mut self,
        memory: &mut GuestMemory<'_>,
        bind: GuestPtr<str>,
    ) -> Result<types::SocketHandle, BlocklessSocketErrorKind> {
        let addr = memory
            .as_str(bind)
            .map_err(|_| BlocklessSocketErrorKind::ParameterError)?
            .unwrap();
        let mode = FileAccessMode::READ | FileAccessMode::WRITE;
        match tcp_bind(&addr)
            .await
            .map(|f| Arc::new(FileEntry::new(f, mode)))
        {
            Ok(f) => {
                let fd_num = self.table().push(f).unwrap();
                let fd = types::SocketHandle::from(fd_num);
                Ok(fd)
            }
            Err(e) => Err(e),
        }
    }

    async fn tcp_connect(
        &mut self,
        memory: &mut GuestMemory<'_>,
        target: GuestPtr<str>,
    ) -> Result<types::SocketHandle, BlocklessSocketErrorKind> {
        let addr = memory
            .as_str(target)
            .map_err(|_| BlocklessSocketErrorKind::ParameterError)?
            .unwrap();
        let mode = FileAccessMode::READ | FileAccessMode::WRITE;
        match tcp_connect(&addr)
            .await
            .map(|f| Arc::new(FileEntry::new(f, mode)))
        {
            Ok(f) => {
                let fd_num = self.table().push(f).unwrap();
                let fd = types::SocketHandle::from(fd_num);
                Ok(fd)
            }
            Err(e) => Err(e),
        }
    }
}
