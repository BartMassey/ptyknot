// Copyright © 2016 Bart Massey
// This work is made available under the "MIT License".
// Please see the file COPYING in this distribution for
// license terms.

//! Child process and pty support.
//!
//! Start a child process running a specified action. The
//! process may have a new pseudo-tty as its controlling
//! terminal, and may have pipes to the master for some of
//! its file descriptors.  The caller receives handles for
//! all of this, along with the process ID of the child. The
//! caller can then later wait for the child to exit by
//! dropping its last reference.

use std::fs::{File, OpenOptions};
use std::io::{Error, Result};
use std::os::unix::io::AsRawFd;

#[cfg(test)]
use std::io::prelude::*;
#[cfg(test)]
use std::io::{BufReader, stdin};

pub mod pty;

use std::os::unix::io::RawFd;

/// Represents a standard file descriptor to be overwritten
/// in the child process.
#[derive(Copy, Clone, Debug)]
pub struct StdFd(RawFd);

impl StdFd {
    /// Get the underlying raw file descriptor.
    pub fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

/// Stdin target for the pty-side interface.
pub fn pty_stdin() -> StdFd {
    StdFd(0)
}

/// Stdout target for the pty-side interface.
pub fn pty_stdout() -> StdFd {
    StdFd(1)
}

/// Stderr target for the pty-side interface.
pub fn pty_stderr() -> StdFd {
    StdFd(2)
}

/// Parent information about the child process.
pub struct PtyKnot {
    /// Child process ID, if it has not already been reaped.
    pub pid: Option<i32>,
}

impl Drop for PtyKnot {
    // When the `PtyKnot` is dropped, its child process is waited for.
    fn drop(&mut self) {
        if let Some(pid) = self.pid.take() {
            let _ = pty::waitpid(pid);
        }
    }
}

/// Return the master side of a ready-to-operate pseudo-terminal.
///
/// # Example
///
/// ```
/// let mut master = ptyknot::make_pty().expect("could not make pty");
/// let slave_name = ptyknot::pty::ptsname(&mut master)
///                  .expect("could not get slave name");
/// println!("{}", slave_name.to_str()
///                .expect("could not convert slave name"));
/// ```
pub fn make_pty() -> Result<File> {
    let master = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ptmx")?;
    pty::grantpt(&master)?;
    pty::unlockpt(&master)?;
    Ok(master)
}

/// Which direction a pipe runs.
pub enum PipeDirection {
    /// Master reads from pipe, slave writes.
    MasterRead,
    /// Master writes to pipe, slave reads.
    MasterWrite,
}

/// Information needed during the pipe plumbing process.
pub struct Plumbing {
    master: File,
    slave: File,
    slave_target: StdFd,
}

impl Plumbing {
    /// Create a new pipe running in the specified
    /// direction, and remember the file descriptor of the
    /// given file. This will later allow the slave to
    /// attach `slave_target` to the other end of the pipe.
    pub fn new(direction: PipeDirection, slave_target: StdFd) -> Result<Plumbing> {
        let [pipefds0, pipefds1] = pty::pipe()?;
        let (master, slave) = match direction {
            PipeDirection::MasterWrite => (pipefds1, pipefds0),
            PipeDirection::MasterRead => (pipefds0, pipefds1),
        };
        Ok(Plumbing {
            master,
            slave,
            slave_target,
        })
    }

    fn install_in_child(self) -> Result<()> {
        use std::os::unix::io::IntoRawFd;
        let Plumbing {
            master,
            slave,
            slave_target,
        } = self;

        drop(master);
        if slave.as_raw_fd() != slave_target.as_raw_fd() {
            pty::dup2(&slave, slave_target)?;
            drop(slave);
        } else {
            let _ = slave.into_raw_fd();
        }
        Ok(())
    }

    fn into_parent(self) -> File {
        let Plumbing {
            master,
            slave,
            slave_target: _,
        } = self;
        drop(slave);
        master
    }
}

struct PreparedPty {
    master: File,
    slave_name: std::path::PathBuf,
}

/// Setup configuration for spawning a child process.
pub struct PtyKnotSetup {
    pty: Option<PreparedPty>,
    plumbing: Vec<Plumbing>,
}

/// Handles retained by the parent process.
pub struct ParentHandles {
    pty: Option<File>,
    pipes: Vec<File>,
}

/// The result of spawning a child process.
pub struct Spawned {
    /// Handles to communicate with the child.
    pub handles: ParentHandles,
    /// Information about the child process.
    pub knot: PtyKnot,
}

impl Spawned {
    /// Consume the result, returning handles and knot.
    pub fn into_parts(self) -> (ParentHandles, PtyKnot) {
        (self.handles, self.knot)
    }
}

impl ParentHandles {
    #[doc(hidden)]
    pub fn into_parts(self) -> (Option<File>, Vec<File>) {
        (self.pty, self.pipes)
    }
}

impl PtyKnotSetup {
    /// Create a new setup configuration.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        PtyKnotSetup {
            pty: None,
            plumbing: Vec::new(),
        }
    }

    /// Add a pre-created PTY to the setup.
    pub fn with_pty(mut self, master: File) -> Result<Self> {
        let slave_name = pty::ptsname(&master)?;
        self.pty = Some(PreparedPty { master, slave_name });
        Ok(self)
    }

    /// Add a pipe to the setup.
    pub fn with_plumbing(mut self, plumbing: Plumbing) -> Self {
        self.plumbing.push(plumbing);
        self
    }

    /// Spawn the child process and return handles to it.
    ///
    /// ```compile_fail
    /// use ptyknot::{make_pty, PtyKnotSetup};
    /// let pty = make_pty().unwrap();
    /// let setup = PtyKnotSetup::new().with_pty(pty).unwrap();
    /// // This should fail to compile because pty is moved into setup.
    /// setup.spawn(|| {
    ///     let _ = pty;
    /// }).unwrap();
    /// ```
    pub fn spawn<F>(self, action: F) -> Result<Spawned>
    where
        F: FnOnce(),
    {
        // # Safety
        // `fork()` has no UB possibilities. It will
        // either succeed or fail.
        let pid = unsafe { libc::fork() };
        match pid {
            -1 => Err(Error::last_os_error()),
            0 => {
                let child_result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                        // Get rid of the current controlling terminal.
                        // # Safety
                        // `setsid()` has no UB possibilities. It will
                        // either succeed or fail.
                        if unsafe { libc::setsid() } == -1 {
                            panic!(
                                "could not lose controlling terminal: {}",
                                Error::last_os_error()
                            );
                        }

                        let _slave = if let Some(prepared) = self.pty {
                            drop(prepared.master);
                            let slave_fd = OpenOptions::new()
                                .read(true)
                                .write(true)
                                .open(prepared.slave_name)
                                .expect("cannot open pty");
                            Some(slave_fd)
                        } else {
                            None
                        };

                        for p in self.plumbing {
                            p.install_in_child().expect("could not plumb pipe");
                        }

                        action();
                    }));

                let status = if child_result.is_ok() { 0 } else { 101 };
                // # Safety
                // This is the child process and no Rust values
                // may be dropped after their descriptors have
                // been closed or reassigned.
                unsafe { libc::_exit(status) }
            }
            pid => {
                let pty = self.pty.map(|p| p.master);
                let pipes = self.plumbing.into_iter().map(|p| p.into_parent()).collect();
                let handles = ParentHandles { pty, pipes };
                let knot = PtyKnot { pid: Some(pid) };
                Ok(Spawned { handles, knot })
            }
        }
    }
}

/// Provide a cleaner interface to the child spawning process.
#[macro_export]
macro_rules! ptyknot {
    ($knot:ident,
     $slave:expr
     $(, @ $tty:ident)*
     $(, < $master_read:ident $read_fd:expr)*
     $(, > $master_write:ident $write_fd:expr)*) => {
        $(let $tty = $crate::make_pty().expect("could not make pty");)*
        $(let $master_read =
          $crate::Plumbing::new($crate::PipeDirection::MasterRead, $read_fd)
          .expect("$master_read: create failed");)*
        $(let $master_write =
          $crate::Plumbing::new($crate::PipeDirection::MasterWrite, $write_fd)
          .expect("$master_write: create failed");)*

        let mut setup = $crate::PtyKnotSetup::new();
        $(
            setup = setup.with_pty($tty).expect("with_pty failed");
        )*
        $(
            setup = setup.with_plumbing($master_read);
        )*
        $(
            setup = setup.with_plumbing($master_write);
        )*

        let spawned = setup.spawn($slave).expect("ptyknot failed");
        let (handles, $knot) = spawned.into_parts();
        let (pty_opt, pipes) = handles.into_parts();
        let mut pipes_iter = pipes.into_iter();

        $(
            #[allow(unused_mut)]
            let mut $tty = pty_opt.expect("missing pty");
        )*
        $(
            let $master_read = pipes_iter.next().expect("missing pipe");
        )*
        $(
            #[allow(unused_mut)]
            let mut $master_write = pipes_iter.next().expect("missing pipe");
        )*
    }
}

#[cfg(test)]
fn pty_slave() {
    let mut tty = OpenOptions::new()
        .write(true)
        .open("/dev/tty")
        .expect("cannot open /dev/tty");
    tty.write_all("hello world\n".as_bytes())
        .expect("cannot write to /dev/tty");
    tty.flush().expect("cannot flush /dev/tty");
}

#[test]
fn pty_test() {
    let pty = make_pty().expect("could not make pty");
    let setup = PtyKnotSetup::new().with_pty(pty).expect("with_pty");
    let spawned = setup.spawn(pty_slave).expect("spawn fail");
    let (handles, knot) = spawned.into_parts();
    let (pty_opt, _pipes) = handles.into_parts();
    let pty = pty_opt.unwrap();

    let mut master = BufReader::new(&pty);
    let mut message = String::new();
    master
        .read_line(&mut message)
        .expect("could not read message");
    drop(knot);
    assert_eq!(message.trim(), "hello world");
}

#[cfg(test)]
fn pipe_slave() {
    // This needs to not be stdout for the test.
    // See https://github.com/rust-lang/rust/issues/35136 .
    writeln!(std::io::stderr(), "hello world").expect("could not write message");
}

#[test]
fn pipe_test() {
    let pipeout =
        Plumbing::new(PipeDirection::MasterRead, pty_stderr()).expect("could not create pipeout");
    let setup = PtyKnotSetup::new().with_plumbing(pipeout);
    let spawned = setup.spawn(pipe_slave).expect("spawn fail");
    let (handles, knot) = spawned.into_parts();
    let (_pty, mut pipes) = handles.into_parts();
    let pipeout = pipes.pop().unwrap();

    let mut master = BufReader::new(pipeout);
    let mut message = String::new();
    master
        .read_line(&mut message)
        .expect("could not read message");
    drop(knot);
    assert_eq!(message.trim(), "hello world");
}

#[cfg(test)]
fn macro_slave() {
    let mut tty = OpenOptions::new()
        .write(true)
        .open("/dev/tty")
        .expect("could not open /dev/tty");
    tty.write_all("hello world\n".as_bytes())
        .expect("could not write /dev/tty");
    tty.flush().expect("cannot flush /dev/tty");
    let mut input = BufReader::new(stdin());
    let mut message = String::new();
    input.read_line(&mut message).expect("could not read stdin");
}

#[test]
pub fn macro_test() {
    ptyknot!(knot, macro_slave, @ child_pty, > child_stdin pty_stdin());
    let mut tty = BufReader::new(&child_pty);
    let mut message = String::new();
    tty.read_line(&mut message).expect("could not read tty");
    writeln!(child_stdin, "hello world\n").expect("could not write stdin");
    // This will wait for the child.
    drop(knot);
}

#[test]
fn plumbing_preserves_parent_standard_descriptors() {
    let descriptors = [pty_stdin(), pty_stdout(), pty_stderr()];
    for descriptor in descriptors {
        let fd_path = format!("/proc/self/fd/{}", descriptor.as_raw_fd());
        assert!(
            std::fs::metadata(&fd_path).is_ok(),
            "standard descriptor was not open before plumbing: {fd_path}"
        );

        let plumbing = Plumbing::new(PipeDirection::MasterRead, descriptor)
            .expect("could not create plumbing");
        drop(plumbing.into_parent());

        assert!(
            std::fs::metadata(&fd_path).is_ok(),
            "plumbing closed parent standard descriptor: {fd_path}"
        );
    }
}

#[test]
fn setup_drop_closes_files() {
    let pty = make_pty().expect("could not make pty");
    let fd = pty.as_raw_fd();
    let setup = PtyKnotSetup::new().with_pty(pty).expect("with_pty");
    drop(setup);

    // Attempting to read fd should fail because it's closed.
    let fd_path = format!("/proc/self/fd/{}", fd);
    assert!(std::fs::metadata(&fd_path).is_err(), "fd was not closed");
}

#[test]
fn child_panic_status() {
    let setup = PtyKnotSetup::new();
    let spawned = setup.spawn(|| panic!("test panic")).expect("spawn fail");
    let (_handles, mut knot) = spawned.into_parts();

    let pid = knot.pid.take().expect("missing child pid");
    let status = pty::waitpid(pid).expect("waitpid failed");

    assert_eq!(status.code(), Some(101), "expected exit status 101");
}

#[test]
fn parent_pipe_order_matches_setup_order() {
    let first = Plumbing::new(PipeDirection::MasterRead, pty_stdout())
        .expect("could not create first pipe");
    let second = Plumbing::new(PipeDirection::MasterRead, pty_stderr())
        .expect("could not create second pipe");
    let expected = [first.master.as_raw_fd(), second.master.as_raw_fd()];

    let spawned = PtyKnotSetup::new()
        .with_plumbing(first)
        .with_plumbing(second)
        .spawn(|| {})
        .expect("spawn failed");
    let (handles, knot) = spawned.into_parts();
    let (_pty, pipes) = handles.into_parts();
    let actual = [pipes[0].as_raw_fd(), pipes[1].as_raw_fd()];

    assert_eq!(actual, expected);
    drop(pipes);
    drop(knot);
}

#[test]
fn closed_standard_descriptor() {
    let test_binary = std::env::current_exe().expect("could not find test binary");
    let status = std::process::Command::new(test_binary)
        .arg("--exact")
        .arg("closed_standard_descriptor_helper")
        .arg("--ignored")
        .arg("--nocapture")
        .env("PTYKNOT_CLOSED_STDIN_HELPER", "1")
        .status()
        .expect("could not run closed-descriptor helper");

    assert!(
        status.success(),
        "closed-descriptor helper failed: {status}"
    );
}

#[test]
#[ignore]
fn closed_standard_descriptor_helper() {
    if std::env::var_os("PTYKNOT_CLOSED_STDIN_HELPER").is_none() {
        return;
    }
    pty::close(pty_stdin().as_raw_fd()).expect("could not close stdin");

    let pipe =
        Plumbing::new(PipeDirection::MasterWrite, pty_stdin()).expect("could not create pipe");
    let spawned = PtyKnotSetup::new()
        .with_plumbing(pipe)
        .spawn(|| {
            let mut input = BufReader::new(stdin());
            let mut message = String::new();
            input.read_line(&mut message).expect("could not read stdin");
            assert_eq!(message.trim(), "hello world");
        })
        .expect("spawn failed");

    let (handles, mut knot) = spawned.into_parts();
    let (_pty, mut pipes) = handles.into_parts();
    let mut pipe = pipes.pop().expect("missing parent pipe");
    writeln!(pipe, "hello world").expect("could not write child stdin");
    drop(pipe);

    let pid = knot.pid.take().expect("missing child pid");
    let status = pty::waitpid(pid).expect("waitpid failed");
    assert!(status.success(), "child failed: {status}");
}
