/* P1 harness for `ori compile --lib` + `@c_export` (PLANO-CDYLIB-EMBED).
 *
 * Build (after compiling the Ori library):
 *   ori compile --lib examples/embed/add_scores.orl -o /tmp/libadd_scores.so
 *   cc -O2 -o /tmp/embed_smoke tests/native/embed_smoke.c \
 *        -ldl -Wl,-rpath,/path/to/runtime/x86_64-unknown-linux-gnu
 *   ORI_EMBED_LIB=/tmp/libadd_scores.so /tmp/embed_smoke
 *
 * Or use: sh tools/qa/embed_smoke.sh
 */
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <time.h>

typedef int64_t (*add_scores_fn)(int64_t, int64_t);
typedef int64_t (*mul_scores_fn)(int64_t, int64_t);
typedef int32_t (*ori_rt_init_fn)(void);
typedef void (*ori_rt_shutdown_fn)(void);
typedef void (*ori_module_init_fn)(void);

static void *must_dlsym(void *h, const char *name) {
    void *p = dlsym(h, name);
    if (!p) {
        fprintf(stderr, "dlsym(%s) failed: %s\n", name, dlerror());
        exit(2);
    }
    return p;
}

int main(int argc, char **argv) {
    const char *lib_path = getenv("ORI_EMBED_LIB");
    if (!lib_path && argc > 1) {
        lib_path = argv[1];
    }
    if (!lib_path) {
        fprintf(stderr, "usage: embed_smoke <lib.so>  (or set ORI_EMBED_LIB)\n");
        return 2;
    }

    void *h = dlopen(lib_path, RTLD_NOW | RTLD_LOCAL);
    if (!h) {
        fprintf(stderr, "dlopen(%s) failed: %s\n", lib_path, dlerror());
        return 2;
    }

    /* Runtime symbols live in libori_runtime.so (NEEDED by the Ori lib). */
    ori_rt_init_fn rt_init = (ori_rt_init_fn)must_dlsym(h, "ori_rt_init");
    ori_rt_shutdown_fn rt_shutdown = (ori_rt_shutdown_fn)must_dlsym(h, "ori_rt_shutdown");
    ori_module_init_fn mod_init =
        (ori_module_init_fn)dlsym(h, "__ori_module_init"); /* optional */

    add_scores_fn add = (add_scores_fn)must_dlsym(h, "add_scores");
    mul_scores_fn mul = (mul_scores_fn)must_dlsym(h, "mul_scores");

    if (rt_init() != 0) {
        fprintf(stderr, "ori_rt_init failed\n");
        return 1;
    }
    if (mod_init) {
        mod_init();
    }

    int64_t sum = add(2, 3);
    if (sum != 5) {
        fprintf(stderr, "add_scores(2,3) = %lld, expected 5\n", (long long)sum);
        return 1;
    }
    int64_t prod = mul(6, 7);
    if (prod != 42) {
        fprintf(stderr, "mul_scores(6,7) = %lld, expected 42\n", (long long)prod);
        return 1;
    }

    /* 1M calls — P1 accept: no crash; rough latency print. */
    const int N = 1000000;
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
    volatile int64_t acc = 0;
    for (int i = 0; i < N; i++) {
        acc += add(i, 1);
    }
    clock_gettime(CLOCK_MONOTONIC, &t1);
    double ns = (t1.tv_sec - t0.tv_sec) * 1e9 + (t1.tv_nsec - t0.tv_nsec);
    double per_call_ns = ns / (double)N;
    printf("embed_smoke: OK  add_scores(2,3)=%lld  mul_scores(6,7)=%lld\n",
           (long long)sum, (long long)prod);
    printf("embed_smoke: %d calls  %.2f ns/call  (acc=%lld)\n", N, per_call_ns,
           (long long)acc);

    rt_shutdown();
    dlclose(h);
    return 0;
}
