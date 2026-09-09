//! Transport-only stream. Remote connections always verify the deployment CA and name.
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned,
};
use std::{
    fs::File,
    io::{self, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::Path,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

pub const LAN_SERVER: &str = "192.168.5.250:6969";
pub const LAN_PORT: u16 = 6969;
const TIMEOUT: Duration = Duration::from_secs(5);
pub const DEPLOYMENT_CA: &[u8] = include_bytes!("../assets/network/server-ca.der");

pub enum GameStream {
    Plain(TcpStream),
    Client(Box<StreamOwned<ClientConnection, TcpStream>>),
    Server(Box<StreamOwned<ServerConnection, TcpStream>>),
}

impl GameStream {
    pub fn connect(address: SocketAddr) -> io::Result<Self> {
        if address.ip().is_loopback() && address.port() != LAN_PORT {
            let socket = TcpStream::connect_timeout(&address, TIMEOUT)?;
            socket.set_nonblocking(true)?;
            socket.set_nodelay(true)?;
            return Ok(Self::Plain(socket));
        }
        Self::connect_tls(address, DEPLOYMENT_CA)
    }

    pub fn connect_tls(address: SocketAddr, ca: &[u8]) -> io::Result<Self> {
        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from(ca.to_vec()))
            .map_err(io::Error::other)?;
        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let mut connection =
            ClientConnection::new(Arc::new(config), ServerName::IpAddress(address.ip().into()))
                .map_err(io::Error::other)?;
        connection.set_buffer_limit(Some(128 * 1024));
        let mut socket = TcpStream::connect_timeout(&address, TIMEOUT)?;
        socket.set_nonblocking(true)?;
        socket.set_nodelay(true)?;
        let started = Instant::now();
        while connection.is_handshaking() {
            match connection.complete_io(&mut socket) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
            if started.elapsed() >= TIMEOUT {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "secure connection timed out",
                ));
            }
            thread::sleep(Duration::from_millis(2));
        }
        Ok(Self::Client(Box::new(StreamOwned::new(connection, socket))))
    }

    pub fn accept(mut socket: TcpStream, config: Option<Arc<ServerConfig>>) -> io::Result<Self> {
        socket.set_nonblocking(true)?;
        socket.set_nodelay(true)?;
        let Some(config) = config else {
            return Ok(Self::Plain(socket));
        };
        let mut connection = ServerConnection::new(config).map_err(io::Error::other)?;
        connection.set_buffer_limit(Some(128 * 1024));
        let started = Instant::now();
        while connection.is_handshaking() {
            match connection.complete_io(&mut socket) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
            if started.elapsed() >= TIMEOUT {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "TLS handshake timed out",
                ));
            }
            thread::sleep(Duration::from_millis(2));
        }
        Ok(Self::Server(Box::new(StreamOwned::new(connection, socket))))
    }

    /// Finish TLS after the final framed response so clean application rejection
    /// is not replaced by rustls' truncation error on the receiving side.
    pub fn close_notify(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(_) => return Ok(()),
            Self::Client(s) => s.conn.send_close_notify(),
            Self::Server(s) => s.conn.send_close_notify(),
        }
        let started = Instant::now();
        loop {
            match self.flush() {
                Err(e)
                    if e.kind() == io::ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_millis(100) =>
                {
                    thread::sleep(Duration::from_millis(2));
                }
                result => return result,
            }
        }
    }

    pub fn set_nonblocking(&self, value: bool) -> io::Result<()> {
        self.socket().set_nonblocking(value)
    }
    pub fn set_nodelay(&self, value: bool) -> io::Result<()> {
        self.socket().set_nodelay(value)
    }
    fn socket(&self) -> &TcpStream {
        match self {
            Self::Plain(s) => s,
            Self::Client(s) => &s.sock,
            Self::Server(s) => &s.sock,
        }
    }
}

impl Read for GameStream {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(b),
            Self::Client(s) => s.read(b),
            Self::Server(s) => s.read(b),
        }
    }
}
impl Write for GameStream {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(b),
            Self::Client(s) => s.write(b),
            Self::Server(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Client(s) => s.flush(),
            Self::Server(s) => s.flush(),
        }
    }
}

pub fn server_config(cert: &Path, key: &Path) -> io::Result<Arc<ServerConfig>> {
    let certs = rustls_pemfile::certs(&mut BufReader::new(File::open(cert)?))
        .collect::<Result<Vec<_>, _>>()?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(File::open(key)?))?
        .ok_or_else(|| io::Error::other("TLS private key missing"))?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(io::Error::other)?;
    Ok(Arc::new(config))
}

/// Existing default and operator tunnel profiles upgrade without an address prompt.
pub fn automatic_endpoint(saved: Option<&str>) -> &str {
    match saved {
        None | Some("127.0.0.1:7777" | "127.0.0.1:17777") => LAN_SERVER,
        Some(value) => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_migrate_without_overwriting_custom_endpoints() {
        for old in [None, Some("127.0.0.1:7777"), Some("127.0.0.1:17777")] {
            assert_eq!(automatic_endpoint(old), LAN_SERVER);
        }
        assert_eq!(automatic_endpoint(Some("127.0.0.1:9999")), "127.0.0.1:9999");
    }
    #[test]
    fn embedded_trust_is_valid_and_invalid_trust_fails_before_connect() {
        let mut roots = RootCertStore::empty();
        assert!(roots
            .add(CertificateDer::from(DEPLOYMENT_CA.to_vec()))
            .is_ok());
        assert!(GameStream::connect_tls("127.0.0.1:1".parse().unwrap(), b"invalid").is_err());
    }
}
