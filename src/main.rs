extern crate libc;

use std::fs::OpenOptions;
use std::io::Error;

// Borrowed from https://github.com/stemjail/tty-rs
mod pty {
    use std::path::*;
    use std::io::{Result, Error};
    use std::os::unix::io::AsRawFd;
    use libc::*;

    const TIOCGPTN: u64 = 0x80045430;

    fn ptsindex<T>(master: &mut T)
       -> Result<u32> where T: AsRawFd {
        let mut idx: c_uint = 0;
        match unsafe { ioctl(master.as_raw_fd(),
                             TIOCGPTN,
                             &mut idx) } {
            0 => Ok(idx),
            _ => Err(Error::last_os_error()),
        }
    }

    pub fn ptsname<T>(master: &mut T) -> Result<PathBuf> where T: AsRawFd {
        Ok(Path::new("/dev/pts")
                 .join(format!("{}", try!(ptsindex(master)))))
    }

    mod raw {
        use libc::c_int;

        extern {
            pub fn grantpt(fd: c_int) -> c_int;
            pub fn unlockpt(fd: c_int) -> c_int;
        }
    }

    pub fn grantpt<T>(master: &mut T) -> Result<()> where T: AsRawFd {
        match unsafe { raw::grantpt(master.as_raw_fd()) } {
            0 => Ok(()),
            _ => Err(Error::last_os_error()),
        }
    }

    pub fn unlockpt<T>(master: &mut T) -> Result<()> where T: AsRawFd {
        match unsafe { raw::unlockpt(master.as_raw_fd()) } {
            0 => Ok(()),
            _ => Err(Error::last_os_error()),
        }
    }
}

pub fn main() {
    let mut master = OpenOptions::new()
                 .read(true).write(true)
                 .open("/dev/ptmx").expect("cannot open ptmx");
    
    pty::grantpt(&mut master).expect("could not grant pty");
    pty::unlockpt(&mut master).expect("could not unlock pty");
    let slave_name = pty::ptsname(&mut master).expect("cannot get pty name");
    println!("{}", slave_name.to_str().as_ref().expect("cannot convert pty name"));
    let slave = OpenOptions::new()
        .read(true).write(true)
        .open(slave_name).expect("cannot open pty");
    match unsafe{ libc::fork() } {
        -1 => {
            panic!("fork failed: {}", Error::last_os_error());
        },
        0 => {
            drop(master)
        },
        _ => {
            drop(slave)
        }
    }
}
