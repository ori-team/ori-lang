#include <stdio.h>
#include <stdlib.h>

typedef struct {
    char *buf;
    size_t offset;
    size_t cap;
    size_t count;
} Arena;

void arena_init(Arena *a, size_t cap) {
    a->buf = (char *)malloc(cap);
    a->offset = 0;
    a->cap = cap;
    a->count = 0;
}

void arena_reset(Arena *a) {
    a->offset = 0;
    a->count = 0;
}

void arena_free(Arena *a) {
    free(a->buf);
}

int main(void) {
    Arena a;
    arena_init(&a, 64 * 1024);
    long total = 0;
    for (int frame = 0; frame < 100000; frame++) {
        arena_reset(&a);
        total += a.count;
    }
    arena_free(&a);
    printf("%ld\n", total);
    return 0;
}
