//! Child process and pty support.
//!
//! Start a child process running a specified action, with
//! a new pseudo-tty as its controlling terminal. Give the
//! caller the master side of that terminal for manipulation,
//! along with the process ID of the child. The caller can
//! then later wait for the child to exit.

extern crate libc;

use std::fs::{OpenOptions, File};
use std::io::{Result, Error};

#[cfg(test)]
use std::io::BufReader;
#[cfg(test)]
use std::io::prelude::*;

pub mod pty;

pub struct PtyKnot {
    pub pid: i32,
    pub pty: File
}

impl Drop for PtyKnot {
    fn drop(&mut self) {
        let status = pty::waitpid(self.pid).expect("could not reap child");
        assert_eq!(status, 0);
    }
}

/// Start a child process running the given action, returning
/// a `PtyKnot` struct for control and communication. When the
/// the structure's destructor is called, it will wait to reap
/// the child process and panic if it has crashed or exited with
/// non-zero status.
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
/// let knot = ptyknot::ptyknot(slave).expect("cannot create slave");
/// let mut master = BufReader::new(&knot.pty);
/// let mut message = String::new();
/// master.read_line(&mut message)
///       .expect("could not read message");
/// ```
pub fn ptyknot<F: Fn()>(action: F) -> Result<PtyKnot> {
    let mut master = OpenOptions::new()
                 .read(true).write(true)
                 .open("/dev/ptmx").expect("cannot open ptmx");
    pty::grantpt(&mut master).expect("could not grant pty");
    pty::unlockpt(&mut master).expect("could not unlock pty");
    let pid = unsafe{ libc::fork() };
    match pid {
        -1 => {
            panic!("fork failed: {}", Error::last_os_error());
        },
        0 => {
            let slave_name = pty::ptsname(&mut master)
                             .expect("cannot get pty name");
            drop(master);
            // Thanks much to
            // https://www.win.tue.nl/~aeb/linux/lk/lk-10.html
            // at "Getting a controlling tty" for helping
            // understand this mess.
            //
            // First, get rid of the current controlling terminal.
            if unsafe { libc::setsid() } == -1 {
                panic!("setsid failed");
            }
            // Now, open the pty, which will set it
            // as the controlling terminal.
            let slave = OpenOptions::new()
                        .read(true).write(true)
                        .open(slave_name).expect("cannot open pty");
            // We are done with the pty now and can close it.
            drop(slave);
            action();
            std::process::exit(0)
        },
        _ => Ok(PtyKnot{pid: pid, pty: master})
    }
}

#[cfg(test)]
fn slave() {
    let mut tty = OpenOptions::new()
                  .write(true)
                  .open("/dev/tty")
                  .expect("cannot open /dev/tty");
    tty.write("hello world\n".as_bytes())
       .expect("cannot write to /dev/tty");
    tty.flush().expect("cannot flush /dev/tty");
}

#[test]
fn it_works() {
    let knot = ptyknot(slave).expect("ptyknot fail");
    let mut master = BufReader::new(&knot.pty);
    let mut message = String::new();
    master.read_line(&mut message)
          .expect("could not read message");
    assert!(message.trim() == "hello world");
}
