extern crate libc;

use std::io::prelude::*;
use std::fs::OpenOptions;
use std::io::{Error, stderr, BufReader};

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

    let pid = unsafe{ libc::fork() };
    match pid {
        -1 => {
            panic!("fork failed: {}", Error::last_os_error());
        },
        0 => {
            stderr().write("slave starting!\n".as_bytes()).expect("oops");
            let slave_name = pty::ptsname(&mut master)
                             .expect("cannot get pty name");
            drop(master);
            // https://www.win.tue.nl/~aeb/linux/lk/lk-10.html
            // at "Getting a controlling tty"
            if unsafe { libc::setsid() } < 0 {
                panic!("setsid failed");
            }
            let mut slave = OpenOptions::new()
                            .read(true).write(true)
                            .open(slave_name).expect("cannot open pty");
            slave.write("hello world\n".as_bytes())
                 .expect("cannot write to /dev/pty");
            slave.flush().expect("cannot flush /dev/pty");
            drop(slave);
            stderr().write("slave sleeping!\n".as_bytes()).expect("oops");
            std::thread::sleep(std::time::Duration::from_secs(1));
            stderr().write("slave exiting!\n".as_bytes()).expect("oops");
        },
        _ => {

            println!("pid: {}", pid);
            let mut master_buf = BufReader::new(&master);
            let mut message = String::new();
            master_buf.read_line(&mut message)
                      .expect("could not read message");
            print!("received message: {}", message);
        }
    }
}
