fn main() {
    let mut v = [1.0f32, 2.0f32, 3.0f32, 4.0f32];
    let step = [0.5f32, 0.25f32, 0.125f32, 0.0625f32];
    for _ in 0..5_000_000 {
        v[0] += step[0];
        v[1] += step[1];
        v[2] += step[2];
        v[3] += step[3];
    }
    println!("{}", std::hint::black_box(v[0] + v[1] + v[2] + v[3]));
}
