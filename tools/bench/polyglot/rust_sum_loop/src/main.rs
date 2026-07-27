fn main() {
    let n: i64 = 10_000_000;
    let mut s: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        s = s.wrapping_add(i);
        i += 1;
    }
    println!("{}", std::hint::black_box(s));
}
