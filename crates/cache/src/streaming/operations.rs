//! High-performance I/O operations for streaming cache
//!
//! Provides zero-copy operations for Linux systems and vectored I/O
//! for scatter-gather operations across all platforms.

use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use std::io;
use std::io::IoSlice;

/// Zero-copy operations for Linux systems
#[cfg(target_os = "linux")]
pub mod zero_copy {
    use std::io;
    use std::os::unix::io::RawFd;

    /// Copy data between file descriptors using sendfile (zero-copy)
    #[allow(dead_code)]
    pub async fn sendfile_copy(from_fd: RawFd, to_fd: RawFd, count: usize) -> io::Result<usize> {
        use libc::{off_t, sendfile};

        let result = unsafe { sendfile(to_fd, from_fd, std::ptr::null_mut::<off_t>(), count) };

        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result as usize)
        }
    }

    /// Copy data using splice for pipe operations (zero-copy)
    #[allow(dead_code)]
    pub async fn splice_copy(from_fd: RawFd, to_fd: RawFd, count: usize) -> io::Result<usize> {
        use libc::{splice, SPLICE_F_MORE, SPLICE_F_MOVE};

        let flags = SPLICE_F_MOVE | SPLICE_F_MORE;
        let result = unsafe {
            splice(
                from_fd,
                std::ptr::null_mut(),
                to_fd,
                std::ptr::null_mut(),
                count,
                flags as std::os::raw::c_uint,
            )
        };

        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result as usize)
        }
    }
}

/// Vectored I/O operations for scatter-gather
pub mod vectored {
    use super::*;

    /// Read into multiple buffers (scatter)
    #[allow(dead_code)]
    pub async fn read_vectored<R: AsyncRead + Unpin>(
        reader: &mut R,
        bufs: &mut [&mut [u8]],
    ) -> io::Result<usize> {
        let mut total = 0;
        for buf in bufs {
            let n = reader.read(buf).await?;
            total += n;
            if n < buf.len() {
                break;
            }
        }
        Ok(total)
    }

    /// Write from multiple buffers (gather)
    #[allow(dead_code)]
    pub async fn write_vectored<W: AsyncWrite + Unpin>(
        writer: &mut W,
        bufs: &[IoSlice<'_>],
    ) -> io::Result<usize> {
        let mut total = 0;
        for buf in bufs {
            let n = writer.write(buf).await?;
            total += n;
            if n < buf.len() {
                break;
            }
        }
        Ok(total)
    }
}
