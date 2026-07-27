fn main() {
    let n: i64 = 2000;
    let mut s: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        let mut j: i64 = 0;
        while j < n {
            s = s.wrapping_add(1);
            j += 1;
        }
        i += 1;
    }
    println!("{}", std::hint::black_box(s));
}
