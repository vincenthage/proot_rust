use std::path::{Path, PathBuf};
use std::usize::MAX as USIZE_MAX;
use errors::{Result, Error};
use register::{Word, Registers};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RED_ZONE_SIZE: isize = 128;
#[cfg(all(target_os = "linux", not(target_arch = "x86_64")))]
const RED_ZONE_SIZE: isize = 0;

pub trait PtraceMemoryAllocator {
    fn alloc_mem(&mut self, size: isize) -> Result<Word>;
}

impl PtraceMemoryAllocator for Registers {
    /// Allocate @size bytes in the @tracee's memory space.
    ///
    /// The register calling this method will have its stack pointer
    /// directly modified. The tracee is not modified.
    /// The registers will have to be pushed for the updates to take place.
    ///
    /// This function should only be called in sysenter since the
    /// stack pointer is systematically restored at the end of
    /// sysexit (except for execve, but in this case the stack
    /// pointer should be handled with care since it is used by the
    /// process to retrieve argc, argv, envp, and auxv).
    ///
    /// `size` can be negative.
    ///
    /// Returns the address of the allocated memory in the @tracee's memory
    /// space, otherwise an error.
    fn alloc_mem(&mut self, size: isize) -> Result<Word> {
        let original_stack_pointer = get_reg!(self.original_regs, StackPointer);

        // Some ABIs specify an amount of bytes after the stack
        // pointer that shall not be used by anything but the compiler
        // (for optimization purpose).
        let corrected_size = match self.stack_pointer == original_stack_pointer {
            false => size,
            true => size + RED_ZONE_SIZE,
        };

        if (corrected_size > 0 && self.stack_pointer <= corrected_size as Word) ||
            (corrected_size < 0 &&
                 self.stack_pointer >= (USIZE_MAX as Word) - (-corrected_size as Word))
        {
            //TODO: log warning
            // note(tracee, WARNING, INTERNAL, "integer under/overflow detected in %s",
            //     __FUNCTION__);
            return Err(Error::bad_address(
                "when allocating memory, under/overflow detected",
            ));
        }

        // Remember the stack grows downward.
        self.stack_pointer = match corrected_size > 0 {
            true => self.stack_pointer - (corrected_size as Word),
            false => self.stack_pointer + (-corrected_size as Word),
        };

        Ok(self.stack_pointer)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;
    use std::usize::MAX;
    use libc::user_regs_struct;
    use nix::unistd::getpid;
    use register::Registers;

    #[test]
    fn test_mem_alloc_normal() {
        let mut raw_regs: user_regs_struct = unsafe { mem::zeroed() };
        let starting_stack_pointer = 100000;

        get_reg!(raw_regs, StackPointer) = starting_stack_pointer;

        let mut regs = Registers::from(getpid(), raw_regs);
        let alloc_size = 7575;
        let new_stack_pointer = regs.alloc_mem(alloc_size).unwrap();

        // Remember the stack grows downward.
        assert!(new_stack_pointer < starting_stack_pointer);
        assert_eq!(
            starting_stack_pointer - new_stack_pointer,
            alloc_size as Word + RED_ZONE_SIZE as Word
        );
    }

    #[test]
    fn test_mem_alloc_overflow() {
        let mut raw_regs: user_regs_struct = unsafe { mem::zeroed() };
        let starting_stack_pointer = 120;

        get_reg!(raw_regs, StackPointer) = starting_stack_pointer;

        let mut regs = Registers::from(getpid(), raw_regs);
        let alloc_size = 7575;
        let result = regs.alloc_mem(alloc_size);

        assert_eq!(
            Err(Error::bad_address(
                "when allocating memory, under/overflow detected",
            )),
            result
        );
    }

    #[test]
    fn test_mem_alloc_underflow() {
        let mut raw_regs: user_regs_struct = unsafe { mem::zeroed() };
        let starting_stack_pointer = (MAX as Word) - 120;

        get_reg!(raw_regs, StackPointer) = starting_stack_pointer;

        let mut regs = Registers::from(getpid(), raw_regs);
        let alloc_size = -7575;
        let result = regs.alloc_mem(alloc_size);

        assert_eq!(
            Err(Error::bad_address(
                "when allocating memory, under/overflow detected",
            )),
            result
        );
    }
}