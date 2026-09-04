use std::sync::mpsc::sync_channel;

fn main() {
    let (tx, rx) = sync_channel::<i64>(1);
    let mut count = 0;
    for i in 0..100_000 {
        tx.send(i).unwrap();
        let v = rx.recv().unwrap();
        if v == i {
            count += 1;
        }
    }
    println!("{}", std::hint::black_box(count));
}
