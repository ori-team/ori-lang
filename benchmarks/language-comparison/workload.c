#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int64_t fib(int64_t n) {
    if (n <= 1) {
        return n;
    }

    int64_t a = 0;
    int64_t b = 1;
    int64_t i = 2;
    while (i <= n) {
        int64_t next = a + b;
        a = b;
        b = next;
        i += 1;
    }
    return b;
}

static int64_t fib_work(int64_t n, int64_t repeat_count) {
    int64_t total = 0;
    int64_t i = 0;
    while (i < repeat_count) {
        total += fib(n);
        i += 1;
    }
    return total;
}

static int64_t sum_squares(int64_t n) {
    int64_t total = 0;
    int64_t i = 1;
    while (i <= n) {
        total += i * i;
        i += 1;
    }
    return total;
}

static int64_t list_push_sum(int64_t n) {
    int64_t *values = (int64_t *)malloc((size_t)n * sizeof(int64_t));
    if (values == NULL) {
        return -1;
    }

    int64_t i = 0;
    while (i < n) {
        values[i] = i * 3 + 7;
        i += 1;
    }

    int64_t total = 0;
    int64_t j = 0;
    while (j < n) {
        total += values[j];
        j += 1;
    }

    free(values);
    return total;
}

int main(void) {
    int64_t fib_acc = fib_work(32, 80000);
    int64_t sum_acc = sum_squares(200000);
    int64_t list_acc = list_push_sum(80000);
    int64_t score = fib_acc + sum_acc + list_acc;

    printf("fib_acc=%lld\n", (long long)fib_acc);
    printf("sum_squares=%lld\n", (long long)sum_acc);
    printf("list_push_sum=%lld\n", (long long)list_acc);
    printf("score=%lld\n", (long long)score);
    return 0;
}
