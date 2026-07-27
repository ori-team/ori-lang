fn main() {
    let n: usize = 1_000_000;
    let mut xs: Vec<i64> = Vec::with_capacity(n);
    let mut i: i64 = 0;
    while (i as usize) < n {
        xs.push(i);
        i += 1;
    }
    let mut s: i64 = 0;
    let mut j = 0usize;
    while j < xs.len() {
        s = s.wrapping_add(xs[j]);
        j += 1;
    }
    println!("{}", std::hint::black_box(s));
}
