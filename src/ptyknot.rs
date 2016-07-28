// Copyright © 2016 Bart Massey
// This work is made available under the "MIT License".
// Please see the file COPYING in this distribution for
// license terms.

//! Child process and pty support.
//!
//! Start a child process running a specified action, with
//! a new pseudo-tty as its controlling terminal. Give the
//! caller the master side of that terminal for manipulation,
//! along with the process ID of the child. The caller can
//! then later wait for the child to exit.

#![warn(missing_docs)]

extern crate libc;

use std::fs::{OpenOptions, File};
use std::io::{Result, Error};
use std::os::unix::io::RawFd;

#[cfg(test)]
use std::io::BufReader;
#[cfg(test)]
use std::io::prelude::*;

pub mod pty;

/// Parent information about the child process.
pub struct PtyKnot {
    /// Child process ID.
    pub pid: i32
}

impl Drop for PtyKnot {
    fn drop(&mut self) {
        let status = pty::waitpid(self.pid).expect("could not reap child");
        assert_eq!(status, 0);
    }
}

/// Return the master side of a ready-to-operate pseudo-terminal.
///
/// # Example
///
/// ```
/// let mut master = ptyknot::make_pty();
/// let slave_name = ptyknot::pty::ptsname(&mut master)
///                  .expect("couldn't get slave name");
/// println!("{}", slave_name.to_str()
///                .expect("couldn't convert slave name"));
/// ```
pub fn make_pty() -> File {
    let mut master = OpenOptions::new()
                 .read(true).write(true)
                 .open("/dev/ptmx").expect("cannot open ptmx");
    pty::grantpt(&mut master).expect("could not grant pty");
    pty::unlockpt(&mut master).expect("could not unlock pty");
    master
}

/// Which direction (from the master's point of view)
/// a pipe runs.
pub enum PipeDirection {
    /// Master reads from pipe, slave writes.
    MasterRead,
    /// Master writes to pipe, slave reads.
    MasterWrite
}

/// Information needed during the pipe plumbing process.
pub struct Plumbing {
    master: RawFd,
    slave: RawFd,
    slave_target: RawFd
}

impl Plumbing {

    /// Create a new pipe running in the specified direction,
    /// and remember the file descriptor of the given file. This
    /// will later allow the slave to attach `slave_target` to the
    /// other end of the pipe.
    pub fn new(direction: PipeDirection, slave_target: RawFd)
           -> Result<Plumbing> {
        let pipefds = try!(pty::pipe());
        let (master, slave) =
            match direction {
                PipeDirection::MasterWrite => (pipefds[1], pipefds[0]),
                PipeDirection::MasterRead => (pipefds[0], pipefds[1])
            };
        Ok(Plumbing {
            master: master,
            slave: slave,
            slave_target: slave_target
        })
    }

    /// Implement the slave side of the plumbing by ensuring
    /// that the slave end of the pipe is attached to the
    /// previously-supplied file descriptor.
    pub fn plumb_slave(&self) -> Result<()> {
        try!(pty::close(self.master));
        pty::dup2(self.slave, self.slave_target)
    }

    /// Extract the master side of the pipe for use by
    /// the parent.
    pub fn get_master(self) -> Result<File> {
        try!(pty::close(self.slave));
        Ok(pty::from_raw_fd(self.master))
    }
}

/// Start a child process running the given action,
/// returning a `PtyKnot` for process information (currently
/// just a process ID). When the the structure's destructor
/// is called, it will wait to reap the child process and
/// panic if it has crashed or exited with non-zero status.
/// 
/// The optional `pty` argument, if supplied with the master
/// side of a pseudoterminal as created by `make_pty()`, will
/// cause the child to be set up with the slave side of that
/// pseudoterminal as its controlling terminal. Otherwise,
/// the child will be set up with no controlling terminal.
///
/// # Examples
///
/// ```
/// use std::fs::OpenOptions;
/// use std::io::{Write, BufRead, BufReader};
///
/// fn slave() {
///     let mut tty = OpenOptions::new()
///                   .write(true)
///                   .open("/dev/tty")
///                   .expect("cannot open /dev/tty");
///     tty.write("hello world\n".as_bytes())
///        .expect("cannot write to /dev/tty");
///     tty.flush().expect("cannot flush /dev/tty");
/// }
/// 
/// let mut pty = ptyknot::make_pty();
/// let knot = ptyknot::ptyknot(Some(&mut pty), slave)
///            .expect("cannot create slave");
/// let mut master = BufReader::new(&pty);
/// let mut message = String::new();
/// master.read_line(&mut message)
///       .expect("could not read message");
/// // This will wait for the child.
/// drop(knot);
/// ```
pub fn ptyknot<F: Fn()>(pty: Option<&mut File>,
                        plumbing: Option<&Vec<&Plumbing>>,
                        action: F)
                        -> Result<PtyKnot> {
    let pid = unsafe{ libc::fork() };
    match pid {
        -1 => {
            panic!("fork failed: {}", Error::last_os_error());
        },
        0 => {
            // Thanks much to
            // https://www.win.tue.nl/~aeb/linux/lk/lk-10.html
            // at "Getting a controlling tty" for helping
            // understand this mess.

            // Get rid of the current controlling terminal.
            if unsafe { libc::setsid() } == -1 {
                panic!("setsid failed");
            }

            // Set a new controlling terminal if desired by
            // opening a pty.
            if let Some(master) = pty {
                let slave_name = pty::ptsname(master)
                                 .expect("cannot get pty name");
                drop(master);
                // Open the pty, which will set it
                // as the controlling terminal.
                let slave = OpenOptions::new()
                            .read(true).write(true)
                            .open(slave_name).expect("cannot open pty");
                // Close the pty, as we are done with it.
                drop(slave);
            }

            // Set up any requested plumbing.
            if let Some(ps) = plumbing {
                for p in ps {
                    p.plumb_slave().expect("couldn't plumb pipe");
                }
            }

            // Run the user action.
            action();
            std::process::exit(0)
        },
        _ => Ok(PtyKnot{pid: pid})
    }
}

#[cfg(test)]
fn pty_slave() {
    let mut tty = OpenOptions::new()
                  .write(true)
                  .open("/dev/tty")
                  .expect("cannot open /dev/tty");
    tty.write("hello world\n".as_bytes())
       .expect("cannot write to /dev/tty");
    tty.flush().expect("cannot flush /dev/tty");
}

#[test]
fn pty_test() {
    let mut pty = make_pty();
    let knot = ptyknot(Some(&mut pty), None, pty_slave)
               .expect("ptyknot fail");
    let mut master = BufReader::new(&pty);
    let mut message = String::new();
    master.read_line(&mut message)
          .expect("could not read message");
    drop(knot);
    assert_eq!(message.trim(), "hello world");
}

#[cfg(test)]
fn pipe_slave() {
    println!("hello world");
}

#[test]
fn pipe_test() {
    let pipeout = Plumbing::new(PipeDirection::MasterRead, 1)
                  .expect("could not create pipeout");
    let knot = ptyknot(None, Some(&vec![&pipeout]), pipe_slave)
               .expect("ptyknot fail");
    let pipeout = pipeout.get_master().expect("could not get master");
    let mut master = BufReader::new(pipeout);
    let mut message = String::new();
    master.read_line(&mut message)
          .expect("could not read message");
    drop(knot);
    assert_eq!(message.trim(), "hello world");
}
