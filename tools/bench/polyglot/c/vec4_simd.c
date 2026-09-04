#include <stdio.h>

typedef float float4 __attribute__((vector_size(16)));

int main(void) {
    float4 v = {1.0f, 2.0f, 3.0f, 4.0f};
    const float4 step = {0.5f, 0.25f, 0.125f, 0.0625f};
    for (int i = 0; i < 5000000; i++) {
        v += step;
    }
    float sum = v[0] + v[1] + v[2] + v[3];
    printf("%.6g\n", sum);
    return 0;
}
