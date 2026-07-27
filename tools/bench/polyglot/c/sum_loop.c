#include <stdio.h>
#include <stdint.h>

int main(void) {
    const int64_t n = 10000000;
    int64_t s = 0;
    int64_t i = 0;
    while (i < n) {
        s += i;
        i += 1;
    }
    printf("%lld\n", (long long)s);
    return 0;
}
