use async_trait::async_trait;
use std::io::{Read, Write};
use std::net::TcpStream;

use std::sync::Mutex;

use super::communication::{Error as CommunicationError, ReadWrite};
use super::runtime::Runtime;

const DEFAULT_ENDPOINT: &str = "127.0.0.1:15423";

pub struct TcpClient {
    stream: Mutex<TcpStream>,
}

impl TcpClient {
    fn new(address: &str) -> Result<Self, std::io::Error> {
        let stream = TcpStream::connect(address)?;
        Ok(Self {
            stream: Mutex::new(stream),
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("connection error")]
    Connect,
}

/// Connect to a running simulator at this endpoint. Endpoint defaults to `127.0.0.1:15423`.
/// This tries to connect repeatedly for up to about 2 seconds.
pub async fn try_connect<R: Runtime>(endpoint: Option<&str>) -> Result<Box<TcpClient>, Error> {
    for _ in 0..200 {
        match TcpClient::new(endpoint.unwrap_or(DEFAULT_ENDPOINT)) {
            Ok(client) => return Ok(Box::new(client)),
            Err(_) => R::sleep(std::time::Duration::from_millis(10)).await,
        }
    }
    Err(Error::Connect)
}

impl crate::util::Threading for TcpClient {}

#[async_trait(?Send)]
impl ReadWrite for TcpClient {
    fn write(&self, msg: &[u8]) -> Result<usize, CommunicationError> {
        let mut stream = self.stream.lock().unwrap();
        stream.write(msg).map_err(|_| CommunicationError::Write)
    }

    async fn read(&self) -> Result<Vec<u8>, CommunicationError> {
        let mut stream = self.stream.lock().unwrap();

        let mut buffer = vec![0; 64];
        let n = stream
            .read(&mut buffer)
            .map_err(|_| CommunicationError::Read)?;
        buffer.truncate(n);
        Ok(buffer)
    }
}
