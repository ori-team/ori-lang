#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

int main(void) {
    const size_t n = 1000000;
    int64_t *xs = (int64_t *)malloc(n * sizeof(int64_t));
    if (!xs) {
        return 1;
    }
    size_t i = 0;
    while (i < n) {
        xs[i] = (int64_t)i;
        i += 1;
    }
    int64_t s = 0;
    i = 0;
    while (i < n) {
        s += xs[i];
        i += 1;
    }
    free(xs);
    printf("%lld\n", (long long)s);
    return 0;
}
