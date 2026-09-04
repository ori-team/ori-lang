#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long queue[2];
    int count = 0;
    for (long i = 0; i < 100000; i++) {
        queue[0] = i;
        long v = queue[0];
        if (v == i) {
            count++;
        }
    }
    printf("%d\n", count);
    return 0;
}
