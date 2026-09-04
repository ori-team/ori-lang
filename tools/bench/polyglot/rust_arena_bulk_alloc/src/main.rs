struct Arena {
    buf: Vec<u8>,
    count: usize,
}

impl Arena {
    fn new(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
            count: 0,
        }
    }
    fn reset(&mut self) {
        self.buf.clear();
        self.count = 0;
    }
}

fn main() {
    let mut a = Arena::new(64 * 1024);
    let mut total: usize = 0;
    for _ in 0..100_000 {
        a.reset();
        total += a.count;
    }
    println!("{}", std::hint::black_box(total));
}
