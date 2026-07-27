#include <stdio.h>
#include <stdint.h>

int main(void) {
    const int64_t n = 2000;
    int64_t s = 0;
    int64_t i = 0;
    while (i < n) {
        int64_t j = 0;
        while (j < n) {
            s += 1;
            j += 1;
        }
        i += 1;
    }
    printf("%lld\n", (long long)s);
    return 0;
}
