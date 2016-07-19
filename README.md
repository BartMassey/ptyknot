# ptyknot
Copyright (c) 2016 Bart Massey

This Rust library provides support for creating a child
process running a specified action, with a new pseudo-tty as
its controlling terminal. The caller gets the master side of
that pseudo-tty for manipulation, along with the process ID
of the child. The caller can then later wait for the child
to exit and examine its exit status.

# Issues

* The name "ptyknot" is stupid.

* This library is quite Linux-specific.

  * It currently requires a Unix-98 `ptmx` pseudo-tty implementation:
    the older BSD-style pseudo-ttys are not yet supported.

  * It currently requires a SysV-style controlling terminal
    implementation that sets a controlling terminal on first
    tty open. The BSD-style `ioctl` to set a controlling
    terminal is not yet supported.

  * There is a comment in the Rust library source that
    indicates that this code will have memory-related
    crashes on some UNIX systems, probably because of broken
    `fork` or thread implementations.

* The API of this library is amateurish and crufty and will
  change in the future. In particular, a `struct` should be
  used to return the information needed by the master side.

* This library is currently aimed at my specific use case,
  and is missing most of what would make it general enough
  to be nice.

  * Support for controlling `stdin`, `stdout` and `stderr`,
    including through pipes, should be added. Rust's
    `std::process::Command` and friends show one likely
    approach. It would be nice to reuse some of that
    functionality, but this looks hard as it is currently
    constructed: `Command::spawn` insists on exec-ing
    immediately after it forks.

  * An option should be provided to allow no controlling
    terminal, or to provide a pre-existing controlling
    terminal.

* The currently-internal `pty` module should be factored
  into a separate library.

* This code needs careful review. It's probably full of all
  kinds of badness.

# Notes

This code does not build with rustc 1.8 because of compiler
and library bugs.

# Credits

This work borrows intensively from
[tty-rs](http://github.com/stemjail/tty-rs).
I only wrote this because I couldn't get that to work with
current Rust in my box, and because I wanted slightly
different functionality.

Specifically, I wrote this library to allow rewriting the
`it_works` test in my reworked
[rustastic-password](http://github.com/conradkleinespel/rustastic-password).
Hopefully I can either get my work pushed upstream there or
get a fork published soon.

Many sources of information were used in coding this. They
are listed in the source code.

# License

This work is made available under the "MIT License".  Please
see the file COPYING in this distribution for license terms.
