#[repr(align(16))]
#[derive(Clone, Copy)]
struct Aabb {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

fn intersect(a: Aabb, b: Aabb) -> bool {
    if a.max_x < b.min_x || a.min_x > b.max_x {
        return false;
    }
    if a.max_y < b.min_y || a.min_y > b.max_y {
        return false;
    }
    true
}

fn main() {
    let target = Aabb {
        min_x: 10.0,
        min_y: 10.0,
        max_x: 20.0,
        max_y: 20.0,
    };
    let mut hits = 0;
    for _ in 0..1_000_000 {
        let probe = Aabb {
            min_x: 15.0,
            min_y: 15.0,
            max_x: 25.0,
            max_y: 25.0,
        };
        if intersect(target, probe) {
            hits += 1;
        }
    }
    println!("{}", std::hint::black_box(hits));
}
