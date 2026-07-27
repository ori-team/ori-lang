fn main() {
    let n: i64 = 20_000_000;
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let t = a.wrapping_add(b);
        a = b;
        b = t;
        i += 1;
    }
    println!("{}", std::hint::black_box(a));
}
