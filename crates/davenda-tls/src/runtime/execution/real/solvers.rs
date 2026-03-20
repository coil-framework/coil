use std::{error::Error, fs, io, net::SocketAddr, path::PathBuf, time::Duration};

use async_trait::async_trait;
use lers::{solver::Solver, solver::SolverHandle, solver::TlsAlpn01Solver};
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
pub(crate) struct FilesystemHttp01Solver {
    root: PathBuf,
}

impl FilesystemHttp01Solver {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn challenge_path(&self, token: &str) -> PathBuf {
        self.root
            .join(".well-known")
            .join("acme-challenge")
            .join(token)
    }
}

#[async_trait]
impl Solver for FilesystemHttp01Solver {
    async fn present(
        &self,
        _domain: String,
        token: String,
        key_authorization: String,
    ) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
        let path = self.challenge_path(&token);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(boxed_io_error)?;
        }
        fs::write(&path, key_authorization).map_err(boxed_io_error)?;
        Ok(())
    }

    async fn cleanup(&self, token: &str) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
        let path = self.challenge_path(token);
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(boxed_io_error(error)),
        }
    }

    fn attempts(&self) -> usize {
        60
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(2)
    }
}

pub(crate) async fn start_tls_alpn_solver(
    address: SocketAddr,
) -> Result<(TlsAlpn01Solver, SolverHandle<io::Error>), io::Error> {
    let solver = TlsAlpn01Solver::new();
    let listener = TcpListener::bind(address).await?;
    let handle = solver.start_with_listener(listener)?;
    Ok((solver, handle))
}

fn boxed_io_error(error: io::Error) -> Box<dyn Error + Send + Sync + 'static> {
    Box::new(error)
}
