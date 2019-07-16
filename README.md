# ptyknot: Run a Rust action in a child process on a virtual terminal
Copyright (c) 2016 Bart Massey

This Rust "crate" provides support for creating a child
process running a specified action, optionally with a new
pseudo-tty as its controlling terminal and with parent pipes
for some of its initial file descriptors. The caller gets
the master side of the pseudo-tty and pipes for
manipulation, along with the process ID of the child. The
caller can then later wait for the child to exit and examine
its exit status.

# Documentation

The
[rustdoc](https://bartmassey.github.io/ptyknot/ptyknot)
is the primary documentation for this crate.

# Issues

* This library is quite Linux-specific.

  * It currently requires a Unix-98 `ptmx` pseudo-tty implementation:
    the older BSD-style pseudo-ttys are not yet supported.

  * It currently requires a SysV-style controlling terminal
    implementation that sets a controlling terminal on first
    tty open. The BSD-style `ioctl` to set a controlling
    terminal is not yet supported.

* This code needs careful review. It's probably full of all
  kinds of badness.

# Credits

This work initially borrowed from
[tty-rs](http://github.com/stemjail/tty-rs).
I only wrote this because I couldn't get that to work with
current Rust in my box, and because I wanted slightly
different functionality.

Specifically, I wrote this library to allow rewriting the
`it_works` test in my reworked version of
[rustastic-password](http://github.com/conradkleinespel/rustastic-password).
`rustastic-password` has long since moved on, so…

Many sources of information were used in coding this. They
are listed in the source code.

# License

This work is made available under the "MIT License".  Please
see the file COPYING in this distribution for license terms.
