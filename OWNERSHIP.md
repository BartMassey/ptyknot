# Ownership-model implementation brief

## Objective

Replace the borrowed pre-fork resource model with an owning
state transition.

The setup phase should own every PTY and pipe descriptor. A
spawn operation should consume that setup and divide its
resources between the parent and child branches. No code should
close a raw descriptor while a live `File` still owns it.

The `ptyknot!()` macro should retain its current syntax and hide
most of the public API change from ordinary users.

## Current problem

The current `ptyknot()` function borrows its resources:

```rust
pub fn ptyknot<F: FnOnce()>(
    action: F,
    pty: Option<&File>,
    plumbing: &[&Plumbing],
) -> Result<PtyKnot>
```

The child closes descriptors obtained through those borrows.
The corresponding `File` values remain alive and still claim to
own the descriptor numbers. Normal child termination hides this
with `_exit`, but the representation does not express the real
resource transition.

The replacement should follow this rule:

> An operation that closes or permanently transforms a resource
> must consume its owner rather than borrow it.

## Recommended public phases

Use ordinary phase-specific types. Generic typestate markers are
not needed because an empty setup is valid and there is only one
important transition.

Suggested types are:

```rust
pub struct PtyKnotSetup {
    pty: Option<PreparedPty>,
    plumbing: Vec<Plumbing>,
}

pub struct ParentHandles {
    pty: Option<File>,
    pipes: Vec<File>,
}

pub struct Spawned {
    pub handles: ParentHandles,
    pub knot: PtyKnot,
}
```

The names may be improved, but keep the three responsibilities
separate:

- `PtyKnotSetup` owns resources before the fork.
- `ParentHandles` owns resources retained by the parent.
- `PtyKnot` owns only child-process state, currently the PID.

Declare `handles` before `knot` in `Spawned`. If a whole
`Spawned` value is dropped, parent pipe handles must close before
`PtyKnot::drop()` waits for the child. Otherwise a child waiting
for pipe EOF could deadlock with the waiting parent.

Do not make `PtyKnot` own the communication handles.

Because an exported macro expands in the downstream crate,
macro code cannot reach private fields. Provide public consuming
accessors such as these, or an equivalent API:

```rust
impl Spawned {
    pub fn into_parts(self) -> (ParentHandles, PtyKnot);
}

impl ParentHandles {
    #[doc(hidden)]
    pub fn into_parts(self) -> (Option<File>, Vec<File>);
}
```

The second method may be hidden from normal documentation while
remaining callable from `$crate` paths in the macro expansion.

## Suggested setup API

A builder-like API should consume and return `self`:

```rust
impl PtyKnotSetup {
    pub fn new() -> Self;

    pub fn with_pty(self, master: File) -> Result<Self>;

    pub fn with_plumbing(self, plumbing: Plumbing) -> Self;

    pub fn spawn<F>(self, action: F) -> Result<Spawned>
    where
        F: FnOnce();
}
```

`with_pty()` returns `Result` because it should resolve and store
the slave name before the fork.

Alternately, constructors may accept all resources at once. The
essential properties are that the setup owns them and `spawn()`
consumes the setup.

The old borrowed `ptyknot()` signature may be replaced. This is
a breaking direct-API change, but the crate has already been
bumped to version 0.5.0 for the descriptor API change.

## Prepared PTY representation

Use a small internal owner:

```rust
struct PreparedPty {
    master: File,
    slave_name: PathBuf,
}
```

Resolve `slave_name` before calling `fork()`. This makes the
branch-specific behavior simple:

- The parent returns `master` in `ParentHandles`.
- The child drops `master`, then opens `slave_name`.

The child should retain the opened slave until after the action,
matching the current race-avoidance behavior.

Moving the master into `PtyKnotSetup` also prevents the child
action from capturing and using it: an attempted capture should
fail at compile time because the value has already been moved.

## Plumbing ownership transitions

`Plumbing` already owns both pipe ends. Give it private consuming
operations instead of a borrowed child-side operation.

The parent transition should resemble:

```rust
fn into_parent(self) -> File {
    let Plumbing {
        master,
        slave,
        slave_target: _,
    } = self;
    drop(slave);
    master
}
```

The child transition should resemble:

```rust
fn install_in_child(self) -> Result<()> {
    let Plumbing {
        master,
        slave,
        slave_target,
    } = self;

    drop(master);
    // Duplicate or transfer `slave` to `slave_target`.
    // Then release the original slave descriptor.
    Ok(())
}
```

The real implementation must handle the case where the slave
descriptor already equals the target descriptor. This can occur
when one of descriptors 0, 1, or 2 was closed before pipe
creation.

If the descriptors differ:

1. Call `dup2`.
2. Drop the original slave `File`.

If they are equal, `dup2` is a no-op and dropping `slave` would
close the desired standard descriptor. Instead, transfer it out
of Rust ownership with the safe `IntoRawFd` operation. The child
process then owns that standard descriptor until `_exit`.

Do not use `mem::forget` when `IntoRawFd` expresses the transfer
directly.

## Fork implementation

`spawn(self, action)` should follow this shape:

```text
prepare all fallible pre-fork information
fork
├── failure: return Err; setup drops normally
├── parent: consume setup into ParentHandles
└── child: consume setup into child resources
           run action
           terminate with _exit
```

Both match arms may consume the setup. They are mutually
exclusive control-flow branches, and after `fork` each process
has its own memory and descriptor table.

In the parent branch:

- Preserve the prepared PTY master.
- Convert every `Plumbing` into its master end.
- Preserve pipe ordering exactly.
- Return `Spawned { handles, knot }`.

In the child branch:

- Drop every parent-side descriptor through normal RAII.
- Open the prepared PTY slave if present.
- Consume and install every `Plumbing`.
- Run the action.
- Catch unwinding at the child boundary.
- Finish with `libc::_exit(0)` on success.
- Use status 101 for a caught panic, matching Rust convention.

Keep the current setup-error behavior unless intentionally
expanding the scope: a child setup failure may panic, be caught,
and result in status 101. An error-reporting pipe back to the
parent would be useful, but is a separate feature.

Once all branch resources are consumed correctly, remove the
raw `pty::close()` helper and its `libc::close` import.

Do not introduce new unsafe code. The existing calls to `fork`,
`setsid`, and `_exit`, plus the low-level FFI wrappers, are the
only expected unsafe operations. `IntoRawFd` is safe and covers
the descriptor-transfer edge case described above.

## Macro compatibility

Preserve the existing invocation syntax:

```rust
ptyknot!(
    knot,
    child_action,
    @ child_pty,
    < child_output pty_stdout(),
    > child_input pty_stdin()
);
```

The macro should:

1. Create all requested resources.
2. Move them into a `PtyKnotSetup`.
3. Call `spawn()`.
4. Destructure the returned `ParentHandles`.
5. Bind the requested names in their original order.

The observable bindings should remain:

- `$knot` is a `PtyKnot`.
- The `@` binding is the parent PTY master `File`.
- A `<` binding is a parent-readable `File`.
- A `>` binding is a parent-writable, mutable `File`.

The macro may use an iterator over the returned pipe vector to
assign repeated bindings. Keep read pipes before write pipes, as
in the current expansion.

Use `$crate::PipeDirection` in every macro branch. Do not depend
on the caller importing `PipeDirection` into scope.

## API behavior to preserve

- `make_pty()` continues to create a usable PTY master.
- A PTY establishes the child's controlling terminal but does
  not implicitly replace standard input, output, or error.
- `MasterRead` means the parent reads and the child writes.
- `MasterWrite` means the parent writes and the child reads.
- `pty_stdin()`, `pty_stdout()`, and `pty_stderr()` remain the
  available standard-descriptor targets.
- Dropping `PtyKnot` continues to reap the child.
- The macro continues to support no PTY, no pipes, or both.

Do not broaden this change into process-status redesign, async
waiting, killing children, or reporting child setup errors.

## Tests

Adapt all existing tests to the owning API and add coverage for
the ownership transition.

At minimum, verify:

1. Existing PTY communication still works.
2. Existing pipe communication still works.
3. The macro still supports combined PTY and stdin plumbing.
4. Parent descriptors 0, 1, and 2 remain open.
5. Dropping a setup without spawning closes its owned files.
6. A fork failure, if it can be injected, drops setup resources.
7. Parent pipe order matches macro declaration order.
8. A child panic is contained and exits rather than unwinding
   through the caller's parent-side state.
9. A pipe created while its target standard descriptor is
   initially closed remains installed in the child.

Run tests that close a standard descriptor inside a separately
forked helper. Never close a descriptor belonging to the Cargo
test harness process without restoring it deterministically.

Consider a `compile_fail` doctest showing that a PTY master moved
into the setup cannot also be captured by the child action.

Avoid tests that can hang indefinitely. If testing EOF or drop
ordering, use a bounded helper or another deterministic escape.

## Completion checks

Before handing off the implementation, run:

```text
cargo test --all-targets
cargo test --doc
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo doc
git diff --check
```

Regenerate the checked-in `docs/` tree after the public API is
final. Do not use `update-docs.sh` because it creates a commit as
a side effect.

Do not commit or push unless explicitly requested. Preserve the
unrelated untracked `.agents/` content.
