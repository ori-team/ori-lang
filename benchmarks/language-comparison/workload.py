def fib(n: int) -> int:
    if n <= 1:
        return n

    a = 0
    b = 1
    i = 2
    while i <= n:
        nxt = a + b
        a = b
        b = nxt
        i += 1
    return b


def fib_work(n: int, repeat_count: int) -> int:
    total = 0
    i = 0
    while i < repeat_count:
        total += fib(n)
        i += 1
    return total


def sum_squares(n: int) -> int:
    total = 0
    i = 1
    while i <= n:
        total += i * i
        i += 1
    return total


def list_push_sum(n: int) -> int:
    values = []
    i = 0
    while i < n:
        values.append(i * 3 + 7)
        i += 1

    total = 0
    j = 0
    while j < len(values):
        total += values[j]
        j += 1
    return total


def main() -> None:
    fib_acc = fib_work(32, 80_000)
    sum_acc = sum_squares(200_000)
    list_acc = list_push_sum(80_000)
    score = fib_acc + sum_acc + list_acc

    print(f"fib_acc={fib_acc}")
    print(f"sum_squares={sum_acc}")
    print(f"list_push_sum={list_acc}")
    print(f"score={score}")


if __name__ == "__main__":
    main()
