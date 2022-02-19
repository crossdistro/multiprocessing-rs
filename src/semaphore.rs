use crate::sharedmem::SharedMemory;
use nix::errno::Errno;
use libc::{sem_init, sem_post, sem_wait, sem_t};
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

pub struct RawSemaphore {
    mem: SharedMemory<sem_t>,
}

impl RawSemaphore {
    pub fn new(value: u32) -> Result<Self, Errno> {
        let mem = SharedMemory::new(unsafe { MaybeUninit::uninit().assume_init() })?;
        let pshared = 1;
        let ret = unsafe { sem_init(mem.as_ptr(), pshared, value) };
        if ret != 0 {
            return Err(Errno::last());
        }
        Ok(RawSemaphore { mem })
    }

    pub fn up(&mut self) -> Result<(), Errno> {
        let ret = unsafe { sem_post(self.mem.as_ptr()) };
        if ret != 0 {
            return Err(Errno::last());
        }
        Ok(())
    }

    pub fn down(&mut self) -> Result<(), Errno> {
        let ret = unsafe { sem_wait(self.mem.as_ptr()) };
        if ret != 0 {
            return Err(Errno::last());
        }
        Ok(())
    }
}

pub struct Mutex<T> {
    sem: RawSemaphore,
    mem: SharedMemory<T>,
}

impl<T> Mutex<T> {
    pub fn new(mem: SharedMemory<T>) -> Result<Self, Errno> {
        let sem = RawSemaphore::new(1)?;
        Ok(Mutex { sem, mem })
    }

    pub fn lock<'a>(&'a mut self) -> Result<MutexGuard<'a, T>, Errno> {
        MutexGuard::new(self)
    }
}

unsafe impl<T> Sync for Mutex<T> {}

pub struct MutexGuard<'a, T> {
    mutex: &'a mut Mutex<T>,
    cleanup: bool,
}

impl<'a, T> MutexGuard<'a, T> {
    fn new(mutex: &'a mut Mutex<T>) -> Result<Self, Errno> {
        mutex.sem.down()?;
        Ok(MutexGuard { mutex, cleanup: true })
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        if self.cleanup {
            self.mutex.sem.up().unwrap();
        }
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.mem.as_ptr() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.mem.as_ptr() }
    }
}

unsafe impl<T> Send for MutexGuard<'_, T> {}

#[cfg(test)]
mod test {
    use crate::semaphore::Mutex;
    use crate::sharedmem::SharedMemory;
    use crate::process::spawn;
    use std::time::Duration;

    struct Data {
        array: [u8; 1024],
    }

    #[test]
    fn mutex_works() {
        let data = Data { array: [0xaa; 1024] };
        let mut mutex = Mutex::new(SharedMemory::new(data).unwrap()).unwrap();
        assert_eq!([0xaa; 1024], mutex.lock().unwrap().array);
        let process = {
            // Pre-lock the mutex only for the child.
            let mut guard = mutex.lock().unwrap();
            // TODO: Turn the cleanup blocker into a reasonable public API. The problem is that we
            // fork the process and then we can only allow cleanup in the child and not in the
            // parent.
            guard.cleanup = false;
            spawn(move || {
                guard.cleanup = true;
                std::thread::sleep(Duration::from_millis(100));
                guard.array = [0x55; 1024];
            })
        };
        assert_eq!([0x55; 1024], mutex.lock().unwrap().array);
        let success = process.join().success();
        assert!(success);
    }
}
