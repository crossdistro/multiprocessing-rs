use nix::unistd::{fork, ForkResult, Pid};
use nix::sys::wait::{waitpid, WaitStatus};
use std::process::exit;

pub fn spawn<F>(f: F) -> JoinHandle
where
    F: FnOnce(),
    // TODO: Send was designed for threads and doesn't account for all the process specifics. It is
    // often too strict as processes do not share the data structures any more. It might also be
    // too loose as processes will fork the resources passed in the closure and double-clean them
    // if they are actually shared between the processes.
    F: Send,
{
    match unsafe { fork().unwrap() } {
        ForkResult::Parent { child: pid } => {
            JoinHandle { pid }
        }
        ForkResult::Child => {
            f();
            exit(0);
        }
    }
}

pub struct JoinHandle {
    pid: Pid,
}

impl JoinHandle {
    pub fn join(self) -> CompletedProcess {
        CompletedProcess { status: waitpid(self.pid, None).unwrap() }
    }
}

pub struct CompletedProcess {
    status: WaitStatus,
}

impl CompletedProcess {
    pub fn success(&self) -> bool {
        self.exitcode() == Some(0)
    }

    fn exitcode(&self) -> Option<i32> {
        match self.status {
            WaitStatus::Exited(_, code) => Some(code),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::process::spawn;
    use std::time::{Instant, Duration};

    #[test]
    fn works() {
        let duration = Duration::from_millis(100);
        let process = spawn(|| {
            std::thread::sleep(duration);
        });
        let start = Instant::now();
        let success = process.join().success();
        let diff = Instant::now() - start - duration;
        assert!(success);
        assert!((-5..5).contains(&(diff.as_millis() as i32)));
    }
}
