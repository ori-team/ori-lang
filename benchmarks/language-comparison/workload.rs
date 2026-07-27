fn fib(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }

    let mut a = 0_i64;
    let mut b = 1_i64;
    let mut i = 2_i64;
    while i <= n {
        let next = a + b;
        a = b;
        b = next;
        i += 1;
    }
    b
}

fn fib_work(n: i64, repeat_count: i64) -> i64 {
    let mut total = 0_i64;
    let mut i = 0_i64;
    while i < repeat_count {
        total += fib(n);
        i += 1;
    }
    total
}

fn sum_squares(n: i64) -> i64 {
    let mut total = 0_i64;
    let mut i = 1_i64;
    while i <= n {
        total += i * i;
        i += 1;
    }
    total
}

fn list_push_sum(n: i64) -> i64 {
    let mut values = Vec::with_capacity(n as usize);
    let mut i = 0_i64;
    while i < n {
        values.push(i * 3 + 7);
        i += 1;
    }

    let mut total = 0_i64;
    let mut j = 0_usize;
    while j < values.len() {
        total += values[j];
        j += 1;
    }
    total
}

fn main() {
    let fib_acc = fib_work(32, 80_000);
    let sum_acc = sum_squares(200_000);
    let list_acc = list_push_sum(80_000);
    let score = fib_acc + sum_acc + list_acc;

    println!("fib_acc={fib_acc}");
    println!("sum_squares={sum_acc}");
    println!("list_push_sum={list_acc}");
    println!("score={score}");
}
