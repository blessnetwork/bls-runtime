use std::{collections::HashMap, pin::Pin, sync::Once, time::Duration};

use bytes::{Buf, Bytes};
use futures_util::StreamExt;
use log::{debug, error};
use reqwest::Response;

use crate::HttpErrorKind;
use futures_core;
use futures_core::Stream;

type StreamInBox = Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>;

struct StreamState {
    stream: StreamInBox,
    buffer: Option<Bytes>,
}

enum HttpCtx {
    Response(Response),
    StreamState(StreamState),
}

/// get the http context
fn get_ctx() -> Option<&'static mut HashMap<u32, HttpCtx>> {
    static mut CTX: Option<HashMap<u32, HttpCtx>> = None;
    static CTX_ONCE: Once = Once::new();
    CTX_ONCE.call_once(|| unsafe {
        CTX = Some(HashMap::new());
    });
    unsafe { CTX.as_mut() }
}

fn increase_fd() -> Option<u32> {
    static mut MAX_HANDLE: u32 = 0;
    unsafe {
        MAX_HANDLE += 1;
        Some(MAX_HANDLE)
    }
}

/// request the url and the return the fd handle.
pub(crate) async fn http_req(url: &str, opts: &str) -> Result<(u32, i32), HttpErrorKind> {
    let json = match json::parse(opts) {
        Ok(o) => o,
        Err(_) => return Err(HttpErrorKind::RequestError),
    };
    let method = match json["method"].as_str() {
        Some(s) => String::from(s),
        None => return Err(HttpErrorKind::RequestError),
    };

    let mut body = None;
    if let Some(b) = json["body"].as_str() {
        body = Some(b.to_string());
    }

    let connect_timeout = json["connectTimeout"].as_u64().map(Duration::from_secs);
    let read_timeout = json["readTimeout"].as_u64().map(Duration::from_secs);

    // build the headers from the options json
    let mut headers = reqwest::header::HeaderMap::new();
    let header_value = &json["headers"];

    // Check if header_value is a valid string
    let header_obj = match json::parse(header_value.as_str().unwrap_or_default()) {
        Ok(o) => o,
        Err(_) => return Err(HttpErrorKind::HeadersValidationError),
    };

    if header_obj.is_object() {
        for (key, value) in header_obj.entries() {
            // Handle possible errors from from_bytes
            let header_name = match reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
                Ok(name) => name,
                Err(_) => return Err(HttpErrorKind::HeadersValidationError),
            };

            // Handle possible errors from from_str
            let header_value =
                match reqwest::header::HeaderValue::from_str(value.as_str().unwrap_or_default()) {
                    Ok(value) => value,
                    Err(_) => return Err(HttpErrorKind::HeadersValidationError),
                };

            headers.insert(header_name, header_value);
        }
    }

    let mut client_builder = reqwest::ClientBuilder::new();
    if connect_timeout.is_some() {
        client_builder = client_builder.connect_timeout(connect_timeout.unwrap());
    }
    if read_timeout.is_some() {
        client_builder = client_builder.timeout(read_timeout.unwrap());
    }
    let client = client_builder.build().unwrap();
    let req_method = method.to_lowercase();
    let req_builder = match req_method.as_str() {
        "get" => client.get(url),
        "post" => client.post(url),
        _ => return Err(HttpErrorKind::RequestError),
    };
    let resp = req_builder
        .headers(headers)
        .body(body.unwrap_or_default())
        .send()
        .await
        .map_err(|e| {
            error!("request send error, {}", e);
            HttpErrorKind::RuntimeError
        })?;
    let status = resp.status().as_u16() as i32;
    let fd = increase_fd().unwrap();
    let ctx = get_ctx().unwrap();
    ctx.insert(fd, HttpCtx::Response(resp));
    Ok((fd, status))
}

/// read from handle
pub(crate) fn http_read_head(fd: u32, head: &str) -> Result<String, HttpErrorKind> {
    let ctx = get_ctx().unwrap();
    let respone = match ctx.get_mut(&fd) {
        Some(HttpCtx::Response(h)) => h,
        Some(HttpCtx::StreamState(_)) => return Err(HttpErrorKind::RuntimeError),
        None => return Err(HttpErrorKind::InvalidHandle),
    };
    let headers = respone.headers();
    match headers.get(head) {
        Some(h) => match h.to_str() {
            Ok(s) => Ok(s.into()),
            Err(_) => Err(HttpErrorKind::InvalidEncoding),
        },
        None => Err(HttpErrorKind::HeaderNotFound),
    }
}

async fn stream_read(state: &mut StreamState, dest: &mut [u8]) -> usize {
    let read_call = |buffer: &mut Bytes, dest: &mut [u8]| -> usize {
        let remaining = buffer.remaining();
        if remaining > 0 {
            let n = dest.len().min(remaining);
            buffer.copy_to_slice(&mut dest[..n]);
        }
        if remaining >= dest.len() {
            return dest.len();
        } else if remaining > 0 {
            return remaining;
        }
        0
    };
    let mut readn = 0;
    loop {
        match state.buffer {
            Some(ref mut buffer) => {
                let n = read_call(buffer, &mut dest[readn..]);
                if n + readn <= dest.len() {
                    readn += n;
                }
                if buffer.remaining() == 0 {
                    state.buffer.take();
                }
                if dest.len() == readn {
                    return readn;
                }
            }
            None => {
                let mut buffer = match state.stream.next().await {
                    Some(Ok(s)) => s,
                    Some(Err(e)) => {
                        debug!("error get message {}", e);
                        return readn;
                    }
                    None => return readn,
                };
                let n = read_call(&mut buffer, &mut dest[readn..]);
                if buffer.remaining() > 0 {
                    state.buffer = Some(buffer);
                }
                if dest.len() == readn + n {
                    return readn + n;
                }
                match (readn + n).cmp(&dest.len()) {
                    std::cmp::Ordering::Less => readn += n,
                    std::cmp::Ordering::Equal => return readn + n,
                    std::cmp::Ordering::Greater => unreachable!("can't be happend!"),
                }
            }
        }
    }
}

pub async fn http_read_body(fd: u32, buf: &mut [u8]) -> Result<u32, HttpErrorKind> {
    let ctx = get_ctx().unwrap();
    match ctx.remove(&fd) {
        Some(HttpCtx::Response(resp)) => {
            let stream = Box::pin(resp.bytes_stream());
            let mut stream_state = StreamState {
                stream,
                buffer: None,
            };
            let readn = stream_read(&mut stream_state, buf).await;
            ctx.insert(fd, HttpCtx::StreamState(stream_state));
            Ok(readn as u32)
        }
        Some(HttpCtx::StreamState(mut stream_state)) => {
            let readn = stream_read(&mut stream_state, buf).await;
            ctx.insert(fd, HttpCtx::StreamState(stream_state));
            Ok(readn as u32)
        }
        None => Err(HttpErrorKind::InvalidHandle),
    }
}

/// close the handle, destroy the memory.
pub(crate) fn http_close(fd: u32) -> Result<(), HttpErrorKind> {
    let ctx = get_ctx().unwrap();
    match ctx.remove(&fd) {
        Some(_) => Ok(()),
        None => Err(HttpErrorKind::InvalidHandle),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::error::HttpErrorKind;
    use bytes::BytesMut;
    use json::JsonValue;
    use reqwest::header::{HeaderMap, HeaderValue};
    use std::task::Poll;
    use tokio::runtime::{Builder, Runtime};

    struct TestStream(Vec<Bytes>);

    impl Stream for TestStream {
        type Item = reqwest::Result<Bytes>;

        fn poll_next(
            self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            let s = self.get_mut().0.pop().map(Ok);
            Poll::Ready(s)
        }
    }

    fn build_headers(json_str: &str) -> Result<HeaderMap, HttpErrorKind> {
        let parsed_json = match json::parse(json_str) {
            Ok(json) => json,
            Err(_) => return Err(HttpErrorKind::HeadersValidationError),
        };

        let headers_value = match &parsed_json["headers"] {
            JsonValue::Object(obj) => obj,
            _ => return Err(HttpErrorKind::HeadersValidationError),
        };

        let mut headers = HeaderMap::new();
        for (key, value) in headers_value.iter() {
            let header_name = match reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
                Ok(name) => name,
                Err(_) => return Err(HttpErrorKind::HeadersValidationError),
            };

            let header_value = match value.as_str() {
                Some(val) => match HeaderValue::from_str(val) {
                    Ok(value) => value,
                    Err(_) => return Err(HttpErrorKind::HeadersValidationError),
                },
                None => return Err(HttpErrorKind::HeadersValidationError),
            };

            headers.insert(header_name, header_value);
        }

        Ok(headers)
    }

    fn get_runtime() -> Runtime {
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        rt
    }

    // Test for valid headers
    #[test]
    fn test_valid_headers() {
        let json_str = r#"
       {
           "headers": {
               "Content-Type": "application/json",
               "Authorization": "Bearer token"
           }
       }
       "#;

        let result = build_headers(json_str);
        assert!(result.is_ok());
        let headers = result.unwrap();
        assert_eq!(headers.get("Content-Type").unwrap(), "application/json");
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer token");
    }

    // Test for invalid JSON headers
    #[test]
    fn test_invalid_json_headers() {
        let json_str = r#"
        {
            "headers": "not a json object"
        }
        "#;

        let result = build_headers(json_str);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), HttpErrorKind::HeadersValidationError);
    }

    // Test for invalid header name
    #[test]
    fn test_invalid_header_name() {
        let json_str = r#"
        {
            "headers": {
                "Invalid Header Name": "value"
            }
        }
        "#;

        let result = build_headers(json_str);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), HttpErrorKind::HeadersValidationError);
    }

    // Test for invalid header value
    #[test]
    fn test_invalid_header_value() {
        let json_str = r#"
        {
            "headers": {
                "Content-Type": "\0"
            }
        }
        "#;

        let result = build_headers(json_str);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), HttpErrorKind::HeadersValidationError);
    }

    #[test]
    fn test_stream_read_full() {
        let rt = get_runtime();
        rt.block_on(async move {
            let data: &[u8] = &[1, 2, 3, 4, 5, 6];
            let bytes = BytesMut::from(data);
            let mut state = StreamState {
                stream: Box::pin(TestStream(vec![bytes.freeze()])),
                buffer: None,
            };
            let mut dest: [u8; 16] = [0; 16];
            let n = stream_read(&mut state, &mut dest[..]).await;
            assert!(n == data.len());
            assert!(data == &dest[..n]);
        });
    }

    #[test]
    fn test_stream_read_2step() {
        let rt = get_runtime();
        rt.block_on(async move {
            let data: &[u8] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
            let bytes = BytesMut::from(data);
            let mut state = StreamState {
                stream: Box::pin(TestStream(vec![bytes.freeze()])),
                buffer: None,
            };
            let mut tmp: [u8; 8] = [0; 8];
            let mut dest: Vec<u8> = Vec::new();
            let mut total = stream_read(&mut state, &mut tmp[..]).await;
            dest.extend(&tmp[..]);
            let n = stream_read(&mut state, &mut tmp[..]).await;
            dest.extend(&tmp[..n]);
            total += n;
            assert!(total == data.len());
            assert!(data == &dest[..total]);
        });
    }

    #[test]
    fn test_stream_read_3step() {
        let rt = get_runtime();
        rt.block_on(async move {
            let data: &[u8] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
            let data2: &[u8] = &[13, 14, 15, 16];
            let mut state = StreamState {
                stream: Box::pin(TestStream(vec![Bytes::from(data2), Bytes::from(data)])),
                buffer: None,
            };
            let mut src: Vec<u8> = Vec::new();
            src.extend(data);
            src.extend(data2);
            let mut tmp: [u8; 8] = [0; 8];
            let mut dest: Vec<u8> = Vec::new();
            let _ = stream_read(&mut state, &mut tmp[..]).await;
            dest.extend(&tmp[..]);
            let n = stream_read(&mut state, &mut tmp[..]).await;
            dest.extend(&tmp[..n]);
            let n = stream_read(&mut state, &mut tmp[..]).await;
            assert!(n == 0);
            assert!(src == dest);
        });
    }
}
