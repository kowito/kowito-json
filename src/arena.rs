use std::cell::RefCell;

/// Thread-local scratchpad for storing tape arrays without allocations in the hot path.
pub struct Scratchpad {
    tape_buffer: Vec<u32>,
}

impl Scratchpad {
    pub fn new(capacity: usize) -> Self {
        Self {
            tape_buffer: vec![0; capacity],
        }
    }

    #[inline(always)]
    pub fn get_mut_tape(&mut self) -> &mut [u32] {
        &mut self.tape_buffer
    }
}

// Global thread-local
thread_local! {
    pub static GLOBAL_SCRATCHPAD: RefCell<Scratchpad> = RefCell::new(Scratchpad::new(1024 * 1024)); // 1M tape entries
}

/// Runs the given closure with a reference to the thread local scratchpad tape buffer.
#[inline(always)]
pub fn with_scratch_tape<F, R>(f: F) -> R
where
    F: FnOnce(&mut [u32]) -> R,
{
    GLOBAL_SCRATCHPAD.with(|pad| {
        let mut pad = pad.borrow_mut();
        f(pad.get_mut_tape())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scratchpad_initialization() {
        let mut pad = Scratchpad::new(10);
        assert_eq!(pad.get_mut_tape().len(), 10);
        assert!(pad.get_mut_tape().iter().all(|&x| x == 0));
    }

    #[test]
    fn test_scratchpad_mutation() {
        let mut pad = Scratchpad::new(5);
        let tape = pad.get_mut_tape();
        tape[0] = 42;
        tape[4] = 99;
        assert_eq!(pad.get_mut_tape()[0], 42);
        assert_eq!(pad.get_mut_tape()[4], 99);
    }

    #[test]
    fn test_thread_local_scratchpad() {
        with_scratch_tape(|tape| {
            assert!(tape.len() >= 1024); // verify minimum size
            tape[0] = 777;
        });

        with_scratch_tape(|tape| {
            assert_eq!(tape[0], 777); // Verify value persists
            // Reset for other tests
            tape[0] = 0;
        });
    }
}
