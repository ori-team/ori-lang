#include <stdio.h>
#include <stdint.h>

int main(void) {
    const int64_t n = 20000000;
    int64_t a = 0;
    int64_t b = 1;
    int64_t i = 0;
    while (i < n) {
        int64_t t = a + b;
        a = b;
        b = t;
        i += 1;
    }
    printf("%lld\n", (long long)a);
    return 0;
}
