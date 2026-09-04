#include <stdalign.h>
#include <stdbool.h>
#include <stdio.h>

typedef struct {
    double min_x, min_y, max_x, max_y;
} __attribute__((aligned(16))) AABB;

static bool intersect(AABB a, AABB b) {
    if (a.max_x < b.min_x || a.min_x > b.max_x) return false;
    if (a.max_y < b.min_y || a.min_y > b.max_y) return false;
    return true;
}

int main(void) {
    AABB target = {10.0, 10.0, 20.0, 20.0};
    int hits = 0;
    for (int i = 0; i < 1000000; i++) {
        AABB probe = {15.0, 15.0, 25.0, 25.0};
        if (intersect(target, probe)) hits++;
    }
    printf("%d\n", hits);
    return 0;
}
