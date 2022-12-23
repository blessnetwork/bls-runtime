use crate::IpfsErrorKind;

use super::{file::FileApi, gen_boundary, HttpRaw};
use crate::read_ext::ReadRemain;

pub struct Api {
    host: String,
    port: u16,
}

pub struct Response {
    pub(crate) status: u16,
    pub(crate) body: Option<Vec<u8>>,
    pub(crate) read_p: usize,
}

impl Response {
    fn new(status: u16, body: Option<Vec<u8>>) -> Self {
        Self {
            status,
            body,
            read_p: 0,
        }
    }

    pub fn copy_body_remain(&mut self, buf: &mut [u8]) -> usize {
        self.copy_remain(buf)
    }
}

impl ReadRemain for Response {
    fn as_bytes_ref(&self) -> Option<&[u8]> {
        self.body.as_ref().map(|v| &v[..])
    }

    fn read_point(&self) -> usize {
        self.read_p
    }

    fn set_read_point(&mut self, point: usize) {
        self.read_p = point;
    }
}

impl Api {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    pub fn file_api(self) -> FileApi {
        FileApi::new(self)
    }

    pub fn build_url(&self, api: &str) -> String {
        format!("http://{}:{}/{}", &self.host, self.port, api)
    }

    pub async fn simple_post(
        &self,
        url: &str,
        args: Option<String>,
    ) -> Result<Response, IpfsErrorKind> {
        let url = self.build_url(url);
        let url = match args {
            Some(ar) => format!("{}?{}", url, ar),
            None => url,
        };
        let client = reqwest::Client::new();
        let resp = client
            .post(url)
            .send()
            .await
            .map_err(|_| IpfsErrorKind::RequestError)?;
        let status = resp.status().as_u16();
        let bytes = resp
            .bytes()
            .await
            .map_err(|_| IpfsErrorKind::RuntimeError)?;
        return Ok(Response::new(status, Some(bytes.to_vec())));
    }

    pub async fn multipart_raw(
        &self,
        url: &str,
        args: Option<String>,
    ) -> Result<HttpRaw, IpfsErrorKind> {
        let url = self.build_url(url);
        let url = match args {
            Some(ar) => format!("{}?{}", url, ar),
            None => url,
        };
        let mut http = HttpRaw::from_url(&url)?;
        let boudary = format!("------------------------{}", gen_boundary());
        http.boundary(Some(boudary));
        http.method("POST");
        http.connect().await?;
        return Ok(http);
    }
}

mod test {
    #[allow(unused)]
    use super::*;

    #[test]
    fn test_copy_body_remain() {
        let val: &[u8] = b"121212121";
        let mut resp = Response::new(200, Some(val.to_vec()));
        let mut buf: [u8; 1024] = [0; 1024];
        let size = resp.copy_body_remain(&mut buf[..]);
        assert!(val.len() == size);
        assert!(val == &buf[..size]);
    }

    #[test]
    fn test_copy_body_remain2() {
        let val: &[u8] = b"12345678912345678912345679123456789";
        let mut resp = Response::new(200, Some(val.to_vec()));
        let mut buf: [u8; 10] = [0; 10];
        let mut v = Vec::<u8>::new();
        loop {
            let size = resp.copy_body_remain(&mut buf[..]);
            if size == 0 {
                break;
            }
            v.extend_from_slice(&buf[0..size]);
        }

        assert!(val.len() == v.len());
        assert!(val == &v[..v.len()]);
    }
}
