use nix::sys::mman::{mmap, munmap, ProtFlags, MapFlags};
use nix::sys::memfd::{memfd_create, MemFdCreateFlag};
use nix::unistd::close;
use nix::errno::Errno;
use std::ffi::CString;
use std::os::raw::c_void;

pub struct SharedMemory<T> {
    mem: *mut T,
}

impl<T> SharedMemory<T> {
    pub fn new(data: T) -> Result<Self, Errno> {
        let addr = std::ptr::null_mut();
        let size = std::mem::size_of::<T>();
        let prot = ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
        let flags = MapFlags::MAP_SHARED | MapFlags::MAP_ANONYMOUS;
        let fd = memfd_create(&CString::new("memfd").unwrap(), MemFdCreateFlag::empty())?;
        let offset = 0;
        // TODO: Clean up memfd even when mmap fails.
        let mem = unsafe { mmap(addr, size, prot, flags, fd, offset)?.cast::<T>() };
        close(fd).unwrap();
        unsafe { *mem = data; }
        Ok(SharedMemory { mem })
    }

    pub fn as_ptr(&self) -> *mut T {
        self.mem
    }
}

impl<T> Drop for SharedMemory<T> {
    fn drop(&mut self) {
        let size = std::mem::size_of::<T>();
        unsafe { munmap(self.mem.cast::<c_void>(), size) }.unwrap();
    }
}

#[cfg(test)]
mod test {
    use super::SharedMemory;
    use nix::unistd::{fork, ForkResult};
    use nix::sys::wait::{waitpid, WaitStatus};

    #[test]
    fn works() {
        let mem = SharedMemory::new([0xaa; 1024]).unwrap();
        assert_eq!([0xaa; 1024], unsafe { *mem.as_ptr() });
        match unsafe { fork().unwrap() } {
            ForkResult::Parent { child } => {
                let ret = waitpid(child, None).unwrap();
                assert_eq!(ret, WaitStatus::Exited(child, 0));
                assert_eq!([0x55; 1024], unsafe { *mem.as_ptr() });
            }
            ForkResult::Child => {
                let data = unsafe { &mut *mem.as_ptr() };
                *data = [0x55; 1024];
            }
        }
    }
}
