#[macro_use]
extern crate ptyknot;
use std::fs::OpenOptions;
use std::io::{Write, BufRead, BufReader};
// use std::thread::sleep;
// use std::time::*;

fn slave() {
    let mut tty = OpenOptions::new()
                  .write(true)
                  .open("/dev/tty")
                  .expect("cannot open /dev/tty");
    tty.write("hello world\n".as_bytes())
       .expect("cannot write to /dev/tty");
    tty.flush().expect("cannot flush /dev/tty");
}

pub fn main() {
    ptyknot!(knot, slave, @ pty);
    let mut tty = BufReader::new(&pty);
    // let delay = Duration::from_millis(100);
    // sleep(delay);
    let mut message = String::new();
    tty.read_line(&mut message)
       .expect("could not read message");
    // This will wait for the child.
    drop(knot);
}
