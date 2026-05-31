#[derive(Debug)]
pub struct Yielder {
    current: usize,
    cycle_size: usize,
}

impl Yielder {
    pub const fn new(cycle_size: usize) -> Self {
        Self { current: 0, cycle_size }
    }

    pub const fn with_default_scale(size: usize) -> Self {
        Self::new(size)
    }

    pub fn next(&mut self) {
        self.current = (self.current + 1) % self.cycle_size;
        if self.current == 0 {
            loom::skip_branch();
        }
    }
}
