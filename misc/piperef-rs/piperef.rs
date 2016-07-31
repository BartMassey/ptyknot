// Copyright © 2016 Bart Massey

// Demonstrate pipes and redirecting child stdout.

extern crate libc;

use std::fs::File;
use std::io::*;
use std::os::unix::io::FromRawFd;

#[macro_use]
mod cmacros;

enum WriteMode {
    C,
    Macro,
    Stderr
}

fn go(write_mode: WriteMode) {
    // Make a new pipe for child stdout.
    let mut pipefds = [0; 2];
    check_cint!(libc::pipe((&mut pipefds).as_mut_ptr()));
    // Fork a child process.
    let pid: libc::pid_t =  try_cint!(libc::fork());
    if pid == 0 {
        // Child: set up stdout.
        check_cint!(libc::close(pipefds[0]));
        check_cint!(libc::setsid());
        // Write "hello world" to stdout.
        match write_mode {
            WriteMode::Macro => {
                check_cint!(libc::dup2(pipefds[1], 1));
                println!("hello world");
            },
            WriteMode::C => {
                check_cint!(libc::dup2(pipefds[1], 1));
                let buf = "hello world".as_bytes();
                let hello = std::ffi::CString::new(buf).unwrap();
                let bufptr = hello.as_ptr() as *const libc::c_void;
                check_cint!(libc::write(1, bufptr, buf.len()));
            },
            WriteMode::Stderr => {
                check_cint!(libc::dup2(pipefds[1], 2));
                writeln!(stderr(), "hello world")
                .expect("couldn't write stderr");
            }
        }
        // Explicitly exit.
        std::process::exit(0);
    }
    // Parent: Set up child pipe.
    check_cint!(libc::close(pipefds[1]));
    let child_stdout: File = unsafe { FromRawFd::from_raw_fd(pipefds[0]) };
    // Read "hello world" from child pipe.
    let mut message = String::new();
    let mut reader = BufReader::new(child_stdout);
    reader.read_line(&mut message)
          .expect("could not read message");
    // Clean up the child.
    let mut wstatus = 0;
    check_cint!(libc::waitpid(pid, &mut wstatus as *mut libc::c_int, 0));
    let code = try_cint!(libc::WEXITSTATUS(wstatus));
    if code != 0 {
        panic!("child exited with status {}\n", code);
    }
    // Check to make sure the received message was correct.
    assert_eq!(message.trim(), "hello world");
}


fn main() {
    go(WriteMode::C);
    go(WriteMode::Macro);
    go(WriteMode::Stderr);
}

#[test]
fn write_c_test() {
    go(WriteMode::C);
}

#[test]
fn write_stderr_test() {
    go(WriteMode::Stderr);
}

#[test]
fn write_macro_test() {
    go(WriteMode::Macro);
}
