use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};
use tokio::net::{UnixListener, UnixStream};

fn uid() -> u32 {
    // geteuid is infallible and does not use caller-owned memory.
    unsafe { libc::geteuid() }
}

fn directory() -> PathBuf {
    PathBuf::from(format!("/tmp/com.xiaoyu.chipdock.{}", uid()))
}

fn check_directory(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() || meta.uid() != uid() || meta.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ChipDock IPC directory must be private to the current user",
        ));
    }
    Ok(())
}

fn check_peer(stream: &UnixStream) -> io::Result<()> {
    if stream.peer_cred()?.uid() != uid() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ChipDock IPC peer is a different user",
        ));
    }
    Ok(())
}

pub struct Listener {
    socket: UnixListener,
    path: PathBuf,
    _lease: File,
}

impl Listener {
    pub fn bind() -> io::Result<Self> {
        Self::bind_in(directory())
    }

    fn bind_in(dir: PathBuf) -> io::Result<Self> {
        match DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        check_directory(&dir)?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(dir.join("control.lock"))?;
        if !lease.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "ChipDock IPC lock is not a regular file",
            ));
        }
        // The lock is held by this descriptor until the listener drops.
        if unsafe { libc::flock(lease.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let path = dir.join("control.sock");
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_socket() && meta.uid() == uid() => {
                fs::remove_file(&path)?
            }
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "ChipDock IPC endpoint is not an owned socket",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let socket = UnixListener::bind(&path)?;
        Ok(Self {
            socket,
            path,
            _lease: lease,
        })
    }

    pub async fn accept(&mut self) -> io::Result<UnixStream> {
        let (stream, _) = self.socket.accept().await?;
        check_peer(&stream)?;
        Ok(stream)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub async fn connect() -> io::Result<UnixStream> {
    let dir = directory();
    check_directory(&dir)?;
    let stream = UnixStream::connect(dir.join("control.sock")).await?;
    check_peer(&stream)?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn second_listener_cannot_replace_the_first_and_peer_is_same_user() {
        let temporary = tempfile::Builder::new()
            .prefix("cd")
            .tempdir_in("/tmp")
            .unwrap();
        let directory = temporary.path().join("private");
        let mut server = Listener::bind_in(directory.clone()).unwrap();
        assert!(Listener::bind_in(directory.clone()).is_err());
        let (client, peer) = tokio::join!(
            UnixStream::connect(directory.join("control.sock")),
            server.accept()
        );
        check_peer(&client.unwrap()).unwrap();
        peer.unwrap();
        drop(server);
        assert!(Listener::bind_in(directory).is_ok());
    }
}
