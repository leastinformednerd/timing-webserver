use libc::{syscall, SYS_pidfd_open, SYS_waitid, P_PIDFD, WEXITED};
use std::fmt::Formatter;
use std::future::Future;
use std::io;
use std::io::ErrorKind;
use std::os::fd::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::unix::AsyncFd;
use tokio::process::{Child, Command};

/// The stored information that we get for each process
/// I will probably include other metrics and expand past just the compute time metrics at some point
pub struct TimingInfo {
    pub user_time: libc::timeval,
    pub sys_time: libc::timeval,
}

// This is needed since libc::timeval doesn't implement Debug for obvious reasons
impl std::fmt::Debug for TimingInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "TimingInfo {{ user_time: (seconds: {}, useconds: {}), sys_time: (seconds: {}, useconds: {}) }}",
            self.user_time.tv_sec, self.user_time.tv_usec, self.sys_time.tv_sec, self.sys_time.tv_usec
        )
    }
}

impl TimingInfo {
    fn from_rusage(ru: &libc::rusage) -> TimingInfo {
        TimingInfo {
            user_time: ru.ru_utime,
            sys_time: ru.ru_stime,
        }
    }
}

/// A future that owns a process and is responsible for cleaning it up when it exits,
/// as well as returning resource usage information when it does so
pub struct TimingFuture {
    async_fd: AsyncFd<RawFd>,

    // Dropping a Child causes it to tell the runtime to reap the process, causing the runtime to
    // race with this to wait for it. By dropping only when the future is dropped we make sure that
    // it's waited for by us, allowing resource usage information collection
    _child: Child,
}

impl Future for TimingFuture {
    type Output = io::Result<TimingInfo>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<TimingInfo>> {
        match self.async_fd.poll_read_ready(cx) {
            Poll::Pending => Poll::Pending,
            // Annoyingly the following doesn't work:
            // err@Poll::Ready(Err(_)) => err
            // presumably because the type checker isn't aware of the match arm that
            // `err` is bound to :(
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Ready(Ok(guard)) => {
                // This is safe since it's never read before being overwritten,
                // and - fwiw - there's no invalid state of this struct, just very short wrong ones
                let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
                let mut status: i32 = 0;

                // This is safe as long the syscall isn't bugged since these pointers shouldn't be
                // preserved and they're definitely valid so this obeys rusts memory model -is safe
                let r = unsafe {
                    libc::syscall(
                        SYS_waitid,
                        P_PIDFD,
                        *guard.get_inner(),
                        &raw mut status,
                        WEXITED,
                        &raw mut ru,
                    )
                };

                if r == -1 {
                    use io::ErrorKind::*;
                    let last_err = io::Error::last_os_error();
                    match last_err.kind() {
                        WouldBlock => {
                            return self.poll(cx);
                        }
                        _kind => {
                            return Poll::Ready(Err(last_err));
                        }
                    }
                }

                Poll::Ready(Ok(TimingInfo::from_rusage(&ru)))
            }
        }
    }
}

fn pidfd_open(pid: i32, flags: u32) -> io::Result<i32> {
    let ret = unsafe { syscall(SYS_pidfd_open, pid, flags) };

    if ret == -1 {
        return Err(io::Error::last_os_error());
    }

    // Seemingly syscalls can return c_long in general, but in this case returns c_int
    // by using "as i32" instead of "as c_int" it's less portable (to weird systems)
    // but idrc this is a personal to
    Ok(ret as i32)
}

/// Spawn a `tokio::process::Command` and ideally return a `TimingFuture` that produces usage info
/// when the child process exits
pub fn timing_spawn(mut cmd: Command) -> io::Result<TimingFuture> {
    let child = cmd.spawn()?;

    let pid = child.id().ok_or(ErrorKind::Other)? as i32;

    Ok(TimingFuture {
        async_fd: AsyncFd::new(pidfd_open(pid, 0).or(Err(ErrorKind::Other))?)?,
        _child: child,
    })
}
