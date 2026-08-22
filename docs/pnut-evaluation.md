# Pnut Protection-Backend Evaluation

**Role:** decision record for Sophia protection-domain backends.

The evaluated source is `mikedanese/pnut` commit
`32044e4a1eb945611686166c5d2422d9325364a7` (`0.2.1`). Pnut is a serious
candidate for a later native backend, but it does not replace Bubblewrap in the
current critical-path tranche.

## What Fits

Pnut already builds the isolation primitives Sophia wants from Rust: user, PID,
mount, network, IPC, UTS, and cgroup namespaces; an explicit mount plan; cleared
environments; descriptor closing and mapping; capability dropping; seccomp;
Landlock; `PR_SET_PDEATHSIG`; and pidfd-based supervision. Its library model is a
better long-term fit than constructing a command line when Sophia needs tighter
kernel policy or richer backend evidence.

The audit found one fail-open configuration edge in its Landlock V4 network
policy. `allowed_bind = []` and `allowed_connect = []` were indistinguishable from
omitted fields, so the natural deny-all spelling disabled handling of that network
operation. The prepared upstream patch makes both fields optional: omission means
unrestricted for compatibility, while an explicit empty list installs the handled
right with no allow rule and therefore denies all. Focused builder, bind, and
connect tests cover the distinction.

## Why Bubblewrap Ships First

Sophia's supervisor needs a nonblocking child handle with three properties at the
same time:

1. the exact host PID or pidfd of the role process for peer-credential admission;
2. `poll`, terminate, and bounded reap under Sophia's existing restart reducer;
3. a PID namespace in which the role becomes PID 1.

Pnut has the necessary clone3 PID and pidfd internally, but its public
`Sandbox::run()` API blocks until the sandbox exits and returns only an exit code.
Its `Once` CLI mode therefore exposes the Pnut supervisor as Sophia's child while
hiding the exact role PID. `Execve` preserves process identity by replacing the
caller, but Pnut correctly rejects PID-namespace isolation in that mode. Neither
mode satisfies all three requirements, and parsing `/proc` around another
supervisor would make a private implementation detail part of Sophia's security
contract.

Bubblewrap 0.11.2 supplies the deployable boundary today. Sophia owns its wrapper
as a process group, discovers the wrapper's single role child through a bounded
startup check, authenticates the role socket against that exact host PID, and
retains normal poll/terminate behavior. The protected broker smoke makes the
filesystem, environment, descriptor, and network claims executable. This is a
backend choice, not a permanent architectural dependency: callers construct a
backend-neutral `ProtectionDomainSpec`.

## Pnut API Needed For Reconsideration

A future Pnut integration should use a public spawn API rather than its CLI. The
minimum useful contract is a child object that owns the pidfd and host PID, reports
the namespace role PID, supports nonblocking status and signal/terminate/reap, and
keeps setup-failure reporting available after spawn. It must also preserve exact
FD mappings and distinguish absent from explicitly empty network allowlists.

When that API exists, compare the two backends with the same Sophia protection
smoke and role-composition tests. Do not weaken PID namespaces, exact socket-peer
admission, or supervisor lifecycle merely to adopt the more capable backend.
