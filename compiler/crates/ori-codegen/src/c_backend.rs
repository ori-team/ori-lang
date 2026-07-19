use ori_ast::expr::{BinaryOp, UnaryOp};
use ori_hir::hir::*;
use ori_types::{substitute_ty_params, DefId, OpaqueTy, Ty};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as FmtWrite;

// ── Runtime header ────────────────────────────────────────────────────────────

const ORI_RUNTIME_H: &str = r#"/* Ori runtime — generated, do not edit */
#include <stdint.h>
#include <inttypes.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <math.h>
#include <time.h>
#if defined(_WIN32)
#include <windows.h>
#else
#include <unistd.h>
#endif

typedef struct { const char* data; size_t len; } ori_string_t;
#define ORI_STR(s) ((ori_string_t){ .data = (s), .len = sizeof(s) - 1 })
#define ORI_STR_PTR(s) ((ori_string_t){ .data = (s), .len = strlen(s) })
static inline void ori_abort_bounds(const char* message) {
    fprintf(stderr, "%s\n", message);
    abort();
}
static inline bool ori_string_eq(ori_string_t a, ori_string_t b) {
    return a.len == b.len && (a.len == 0 || memcmp(a.data, b.data, a.len) == 0);
}
static inline ori_string_t ori_string_concat(ori_string_t a, ori_string_t b) {
    size_t len = a.len + b.len;
    char* out = (char*)malloc(len + 1);
    if (!out) abort();
    if (a.len > 0) {
        memcpy(out, a.data, a.len);
    }
    if (b.len > 0) {
        memcpy(out + a.len, b.data, b.len);
    }
    out[len] = '\0';
    return (ori_string_t){ .data = out, .len = len };
}
static inline ori_string_t ori_string_slice(ori_string_t s, int64_t start, int64_t end) {
    if (start < 0 || end < start || end > (int64_t)s.len) {
        ori_abort_bounds("ori string slice bounds out of range");
    }
    size_t len = (size_t)(end - start);
    char* out = (char*)malloc(len + 1);
    if (!out) abort();
    if (len > 0) {
        memcpy(out, s.data + start, len);
    }
    out[len] = '\0';
    return (ori_string_t){ .data = out, .len = len };
}
static inline ori_string_t ori_string_get(ori_string_t s, int64_t index) {
    if (index < 0 || index >= (int64_t)s.len) {
        ori_abort_bounds("ori string slice bounds out of range");
    }
    return ori_string_slice(s, index, index + 1);
}
static inline int64_t ori_mem_string_as_ptr(ori_string_t s) {
    return (int64_t)(intptr_t)s.data;
}
static inline int64_t ori_mem_string_len(ori_string_t s) {
    return (int64_t)s.len;
}

typedef struct { uint8_t _; } ori_unit_t;
typedef struct { int64_t __start; int64_t __end; } ori_range_t;
typedef struct { bool has_value; } ori_none_t;
#define ORI_NONE ((ori_none_t){ .has_value = false })
typedef struct { bool has_value; int64_t value; } ori_opt_i64_t;
typedef struct { bool has_value; ori_string_t value; } ori_opt_str_t;

typedef struct { void* obj; void* vtable; } ori_any_t;
typedef struct ori_closure {
    void* fn_ptr;
    void* env_ptr;
} ori_closure_t;

/* Dynamic list */
typedef struct { void* data; size_t len; size_t cap; size_t version; size_t elem_size; } ori_list_t;
typedef struct { ori_list_t _f0; ori_list_t _f1; } ori_tuple_list_i64_list_i64_t;
static inline ori_list_t ori_list_new(size_t elem_size) {
    return (ori_list_t){ .data = NULL, .len = 0, .cap = 0, .version = 0, .elem_size = elem_size };
}
static inline void ori_list_reserve(ori_list_t* l, int64_t capacity) {
    if (!l || capacity <= (int64_t)l->cap) return;
    size_t next = l->cap ? l->cap : 4;
    while ((int64_t)next < capacity) next = next ? next * 2 : 4;
    l->data = realloc(l->data, next * l->elem_size);
    l->cap = next;
}
static inline int64_t ori_list_capacity(ori_list_t* l) {
    return l ? (int64_t)l->cap : 0;
}
static inline ori_list_t ori_list_with_capacity(size_t elem_size, int64_t capacity) {
    ori_list_t l = ori_list_new(elem_size);
    ori_list_reserve(&l, capacity);
    return l;
}
static inline void ori_list_push(ori_list_t* l, const void* elem) {
    if (l->len >= l->cap) {
        ori_list_reserve(l, (int64_t)(l->cap ? l->cap * 2 : 4));
    }
    memcpy((char*)l->data + l->len * l->elem_size, elem, l->elem_size);
    l->len++;
    l->version++;
}
static inline void* ori_list_at(ori_list_t* l, int64_t index) {
    if (!l || index < 0 || index >= (int64_t)l->len) {
        ori_abort_bounds("ori list index out of bounds");
    }
    return (char*)l->data + (size_t)index * l->elem_size;
}
static inline ori_list_t ori_list_slice(ori_list_t l, int64_t start, int64_t end) {
    if (start < 0 || end < start || end > (int64_t)l.len) {
        ori_abort_bounds("ori list slice bounds out of range");
    }
    ori_list_t out = ori_list_new(l.elem_size);
    for (int64_t i = start; i < end; i++) {
        ori_list_push(&out, ori_list_at(&l, i));
    }
    return out;
}

/* Small C/debug map and set runtime. This favors clear generated C over the
   native runtime's optimized hash tables. */
typedef enum {
    ORI_KEY_UNKNOWN = 0,
    ORI_KEY_INT = 1,
    ORI_KEY_STRING = 2,
} ori_key_kind_t;
typedef struct {
    int64_t int_value;
    ori_string_t string_value;
} ori_key_t;
typedef struct {
    ori_key_t* items;
    size_t len;
    size_t cap;
    size_t version;
    ori_key_kind_t key_kind;
} ori_set_t;
typedef struct {
    ori_key_t key;
    void* value;
    size_t value_size;
} ori_map_entry_t;
typedef struct {
    ori_map_entry_t* entries;
    size_t len;
    size_t cap;
    size_t version;
    ori_key_kind_t key_kind;
} ori_map_t;
static inline ori_key_t ori_key_int(int64_t value) {
    return (ori_key_t){ .int_value = value, .string_value = ORI_STR("") };
}
static inline ori_key_t ori_key_string(ori_string_t value) {
    return (ori_key_t){ .int_value = 0, .string_value = value };
}
static inline bool ori_key_eq(ori_key_kind_t kind, ori_key_t left, ori_key_t right) {
    return kind == ORI_KEY_STRING
        ? ori_string_eq(left.string_value, right.string_value)
        : left.int_value == right.int_value;
}
static inline void* ori_value_box(const void* value, size_t size) {
    size_t actual_size = size ? size : 1;
    void* out = malloc(actual_size);
    if (!out) abort();
    if (size) {
        memcpy(out, value, size);
    } else {
        memset(out, 0, actual_size);
    }
    return out;
}
static inline bool ori_set_prepare_kind(ori_set_t* set, ori_key_kind_t kind) {
    if (!set) return false;
    if (set->key_kind == ORI_KEY_UNKNOWN) {
        set->key_kind = kind;
        return true;
    }
    return set->key_kind == kind;
}
static inline bool ori_map_prepare_kind(ori_map_t* map, ori_key_kind_t kind) {
    if (!map) return false;
    if (map->key_kind == ORI_KEY_UNKNOWN) {
        map->key_kind = kind;
        return true;
    }
    return map->key_kind == kind;
}
static inline void ori_set_reserve(ori_set_t* set, int64_t capacity) {
    if (!set || capacity <= (int64_t)set->cap) return;
    size_t new_cap = (size_t)capacity;
    set->items = (ori_key_t*)realloc(set->items, new_cap * sizeof(ori_key_t));
    if (!set->items) abort();
    set->cap = new_cap;
}
static inline void ori_map_reserve(ori_map_t* map, int64_t capacity) {
    if (!map || capacity <= (int64_t)map->cap) return;
    size_t new_cap = (size_t)capacity;
    map->entries = (ori_map_entry_t*)realloc(map->entries, new_cap * sizeof(ori_map_entry_t));
    if (!map->entries) abort();
    map->cap = new_cap;
}
static inline ori_set_t* ori_set_new(void) {
    ori_set_t* set = (ori_set_t*)calloc(1, sizeof(ori_set_t));
    if (!set) abort();
    set->cap = 4;
    set->items = (ori_key_t*)calloc(set->cap, sizeof(ori_key_t));
    if (!set->items) abort();
    set->key_kind = ORI_KEY_UNKNOWN;
    set->version = 0;
    return set;
}
static inline int64_t ori_set_len(ori_set_t* set) {
    return set ? (int64_t)set->len : 0;
}
static inline int64_t ori_set_capacity(ori_set_t* set) {
    return set ? (int64_t)set->cap : 0;
}
static inline bool ori_set_is_empty(ori_set_t* set) {
    return ori_set_len(set) == 0;
}
static inline bool ori_set_contains_key(ori_set_t* set, ori_key_kind_t kind, ori_key_t key) {
    if (!set || (set->key_kind != ORI_KEY_UNKNOWN && set->key_kind != kind)) return false;
    for (size_t i = 0; i < set->len; i++) {
        if (ori_key_eq(kind, set->items[i], key)) return true;
    }
    return false;
}
static inline void ori_set_add_key(ori_set_t* set, ori_key_kind_t kind, ori_key_t key) {
    if (!ori_set_prepare_kind(set, kind) || ori_set_contains_key(set, kind, key)) return;
    if (set->len >= set->cap) {
        ori_set_reserve(set, (int64_t)(set->cap ? set->cap * 2 : 4));
    }
    set->items[set->len++] = key;
    set->version++;
}
static inline void ori_set_add(ori_set_t* set, int64_t value) {
    ori_set_add_key(set, ORI_KEY_INT, ori_key_int(value));
}
static inline void ori_set_add_string(ori_set_t* set, ori_string_t value) {
    ori_set_add_key(set, ORI_KEY_STRING, ori_key_string(value));
}
static inline bool ori_set_contains(ori_set_t* set, int64_t value) {
    return ori_set_contains_key(set, ORI_KEY_INT, ori_key_int(value));
}
static inline bool ori_set_contains_string(ori_set_t* set, ori_string_t value) {
    return ori_set_contains_key(set, ORI_KEY_STRING, ori_key_string(value));
}
static inline int64_t ori_set_value_at(ori_set_t* set, int64_t index) {
    if (!set || index < 0 || index >= (int64_t)set->len) return 0;
    return set->items[(size_t)index].int_value;
}
static inline ori_string_t ori_set_string_at(ori_set_t* set, int64_t index) {
    if (!set || index < 0 || index >= (int64_t)set->len) return ORI_STR("");
    return set->items[(size_t)index].string_value;
}
static inline void ori_set_remove_key(ori_set_t* set, ori_key_kind_t kind, ori_key_t key) {
    if (!set || (set->key_kind != ORI_KEY_UNKNOWN && set->key_kind != kind)) return;
    for (size_t i = 0; i < set->len; i++) {
        if (ori_key_eq(kind, set->items[i], key)) {
            set->items[i] = set->items[set->len - 1];
            set->len--;
            set->version++;
            return;
        }
    }
}
static inline void ori_set_remove(ori_set_t* set, int64_t value) {
    ori_set_remove_key(set, ORI_KEY_INT, ori_key_int(value));
}
static inline void ori_set_remove_string(ori_set_t* set, ori_string_t value) {
    ori_set_remove_key(set, ORI_KEY_STRING, ori_key_string(value));
}
static inline bool ori_set_try_remove(ori_set_t* set, int64_t value) {
    bool found = ori_set_contains(set, value);
    if (found) ori_set_remove(set, value);
    return found;
}
static inline bool ori_set_try_remove_string(ori_set_t* set, ori_string_t value) {
    bool found = ori_set_contains_string(set, value);
    if (found) ori_set_remove_string(set, value);
    return found;
}
static inline void ori_set_clear(ori_set_t* set) {
    if (!set) return;
    set->len = 0;
    set->key_kind = ORI_KEY_UNKNOWN;
    set->version++;
}
static inline ori_set_t* ori_set_clone(ori_set_t* set) {
    ori_set_t* out = ori_set_new();
    if (!set) return out;
    ori_set_reserve(out, (int64_t)set->len);
    out->key_kind = set->key_kind;
    for (size_t i = 0; i < set->len; i++) {
        out->items[out->len++] = set->items[i];
    }
    return out;
}
static inline ori_list_t ori_set_to_list(ori_set_t* set) {
    size_t elem_size = set && set->key_kind == ORI_KEY_STRING ? sizeof(ori_string_t) : sizeof(int64_t);
    ori_list_t out = ori_list_new(elem_size);
    if (!set) return out;
    for (size_t i = 0; i < set->len; i++) {
        if (set->key_kind == ORI_KEY_STRING) {
            ori_list_push(&out, &set->items[i].string_value);
        } else {
            ori_list_push(&out, &set->items[i].int_value);
        }
    }
    return out;
}
static inline ori_set_t* ori_set_from_list(ori_list_t list) {
    ori_set_t* out = ori_set_new();
    for (size_t i = 0; i < list.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&list, (int64_t)i));
        ori_set_add(out, item);
    }
    return out;
}
static inline void ori_set_free(ori_set_t* set) {
    if (!set) return;
    free(set->items);
    free(set);
}
static inline ori_set_t* ori_set_union(ori_set_t* left, ori_set_t* right) {
    ori_set_t* out = ori_set_clone(left);
    if (!right) return out;
    for (size_t i = 0; i < right->len; i++) {
        ori_set_add_key(out, right->key_kind == ORI_KEY_UNKNOWN ? ORI_KEY_INT : right->key_kind, right->items[i]);
    }
    return out;
}
static inline ori_set_t* ori_set_intersection(ori_set_t* left, ori_set_t* right) {
    ori_set_t* out = ori_set_new();
    if (!left || !right) return out;
    ori_key_kind_t kind = left->key_kind == ORI_KEY_UNKNOWN ? ORI_KEY_INT : left->key_kind;
    for (size_t i = 0; i < left->len; i++) {
        if (ori_set_contains_key(right, kind, left->items[i])) {
            ori_set_add_key(out, kind, left->items[i]);
        }
    }
    return out;
}
static inline ori_set_t* ori_set_difference(ori_set_t* left, ori_set_t* right) {
    ori_set_t* out = ori_set_new();
    if (!left) return out;
    ori_key_kind_t kind = left->key_kind == ORI_KEY_UNKNOWN ? ORI_KEY_INT : left->key_kind;
    for (size_t i = 0; i < left->len; i++) {
        if (!right || !ori_set_contains_key(right, kind, left->items[i])) {
            ori_set_add_key(out, kind, left->items[i]);
        }
    }
    return out;
}
static inline ori_map_t* ori_map_new(void) {
    ori_map_t* map = (ori_map_t*)calloc(1, sizeof(ori_map_t));
    if (!map) abort();
    map->cap = 4;
    map->entries = (ori_map_entry_t*)calloc(map->cap, sizeof(ori_map_entry_t));
    if (!map->entries) abort();
    map->key_kind = ORI_KEY_UNKNOWN;
    map->version = 0;
    return map;
}
static inline int64_t ori_map_len(ori_map_t* map) {
    return map ? (int64_t)map->len : 0;
}
static inline int64_t ori_map_capacity(ori_map_t* map) {
    return map ? (int64_t)map->cap : 0;
}
static inline bool ori_map_is_empty(ori_map_t* map) {
    return ori_map_len(map) == 0;
}
static inline int64_t ori_map_find(ori_map_t* map, ori_key_kind_t kind, ori_key_t key) {
    if (!map || (map->key_kind != ORI_KEY_UNKNOWN && map->key_kind != kind)) return -1;
    for (size_t i = 0; i < map->len; i++) {
        if (ori_key_eq(kind, map->entries[i].key, key)) return (int64_t)i;
    }
    return -1;
}
static inline void ori_map_store_value(ori_map_entry_t* entry, const void* value, size_t value_size) {
    free(entry->value);
    entry->value = ori_value_box(value, value_size);
    entry->value_size = value_size;
}
static inline void ori_map_set_value_for_key(ori_map_t* map, ori_key_kind_t kind, ori_key_t key, const void* value, size_t value_size) {
    if (!ori_map_prepare_kind(map, kind)) return;
    int64_t existing = ori_map_find(map, kind, key);
    if (existing >= 0) {
        ori_map_store_value(&map->entries[(size_t)existing], value, value_size);
        map->version++;
        return;
    }
    if (map->len >= map->cap) {
        ori_map_reserve(map, (int64_t)(map->cap ? map->cap * 2 : 4));
    }
    ori_map_entry_t* entry = &map->entries[map->len++];
    memset(entry, 0, sizeof(*entry));
    entry->key = key;
    ori_map_store_value(entry, value, value_size);
    map->version++;
}
static inline void ori_map_set_value(ori_map_t* map, int64_t key, const void* value, size_t value_size) {
    ori_map_set_value_for_key(map, ORI_KEY_INT, ori_key_int(key), value, value_size);
}
static inline void ori_map_set_string_value(ori_map_t* map, ori_string_t key, const void* value, size_t value_size) {
    ori_map_set_value_for_key(map, ORI_KEY_STRING, ori_key_string(key), value, value_size);
}
static inline void ori_map_set(ori_map_t* map, int64_t key, int64_t value) {
    ori_map_set_value(map, key, &value, sizeof(value));
}
static inline void ori_map_set_string(ori_map_t* map, ori_string_t key, int64_t value) {
    ori_map_set_string_value(map, key, &value, sizeof(value));
}
static inline void* ori_map_get_value_for_key(ori_map_t* map, ori_key_kind_t kind, ori_key_t key) {
    int64_t index = ori_map_find(map, kind, key);
    return index >= 0 ? map->entries[(size_t)index].value : NULL;
}
static inline void* ori_map_get_value(ori_map_t* map, int64_t key) {
    return ori_map_get_value_for_key(map, ORI_KEY_INT, ori_key_int(key));
}
static inline void* ori_map_get_string_value(ori_map_t* map, ori_string_t key) {
    return ori_map_get_value_for_key(map, ORI_KEY_STRING, ori_key_string(key));
}
static inline int64_t ori_map_get(ori_map_t* map, int64_t key) {
    void* value = ori_map_get_value(map, key);
    return value ? *((int64_t*)value) : 0;
}
static inline int64_t ori_map_get_string(ori_map_t* map, ori_string_t key) {
    void* value = ori_map_get_string_value(map, key);
    return value ? *((int64_t*)value) : 0;
}
static inline bool ori_map_contains(ori_map_t* map, int64_t key) {
    return ori_map_find(map, ORI_KEY_INT, ori_key_int(key)) >= 0;
}
static inline bool ori_map_contains_string(ori_map_t* map, ori_string_t key) {
    return ori_map_find(map, ORI_KEY_STRING, ori_key_string(key)) >= 0;
}
static inline int64_t ori_map_key_at(ori_map_t* map, int64_t index) {
    if (!map || index < 0 || index >= (int64_t)map->len) return 0;
    return map->entries[(size_t)index].key.int_value;
}
static inline ori_string_t ori_map_key_string_at(ori_map_t* map, int64_t index) {
    if (!map || index < 0 || index >= (int64_t)map->len) return ORI_STR("");
    return map->entries[(size_t)index].key.string_value;
}
static inline void* ori_map_value_at(ori_map_t* map, int64_t index) {
    if (!map || index < 0 || index >= (int64_t)map->len) return NULL;
    return map->entries[(size_t)index].value;
}
static inline void ori_map_remove_key(ori_map_t* map, ori_key_kind_t kind, ori_key_t key) {
    int64_t index = ori_map_find(map, kind, key);
    if (index < 0) return;
    size_t i = (size_t)index;
    free(map->entries[i].value);
    map->entries[i] = map->entries[map->len - 1];
    map->len--;
    map->version++;
}
static inline void ori_map_remove(ori_map_t* map, int64_t key) {
    ori_map_remove_key(map, ORI_KEY_INT, ori_key_int(key));
}
static inline void ori_map_remove_string(ori_map_t* map, ori_string_t key) {
    ori_map_remove_key(map, ORI_KEY_STRING, ori_key_string(key));
}
static inline ori_opt_i64_t ori_map_try_get(ori_map_t* map, int64_t key) {
    void* value = ori_map_get_value(map, key);
    return value ? (ori_opt_i64_t){ .has_value = true, .value = *((int64_t*)value) } : (ori_opt_i64_t){ .has_value = false, .value = 0 };
}
static inline ori_opt_i64_t ori_map_try_get_string(ori_map_t* map, ori_string_t key) {
    void* value = ori_map_get_string_value(map, key);
    return value ? (ori_opt_i64_t){ .has_value = true, .value = *((int64_t*)value) } : (ori_opt_i64_t){ .has_value = false, .value = 0 };
}
static inline ori_opt_i64_t ori_map_try_remove(ori_map_t* map, int64_t key) {
    ori_opt_i64_t value = ori_map_try_get(map, key);
    if (value.has_value) ori_map_remove(map, key);
    return value;
}
static inline ori_opt_i64_t ori_map_try_remove_string(ori_map_t* map, ori_string_t key) {
    ori_opt_i64_t value = ori_map_try_get_string(map, key);
    if (value.has_value) ori_map_remove_string(map, key);
    return value;
}
static inline void ori_map_clear(ori_map_t* map) {
    if (!map) return;
    for (size_t i = 0; i < map->len; i++) {
        free(map->entries[i].value);
    }
    map->len = 0;
    map->key_kind = ORI_KEY_UNKNOWN;
    map->version++;
}
static inline ori_map_t* ori_map_clone(ori_map_t* map) {
    ori_map_t* out = ori_map_new();
    if (!map) return out;
    ori_map_reserve(out, (int64_t)map->len);
    out->key_kind = map->key_kind;
    for (size_t i = 0; i < map->len; i++) {
        ori_map_entry_t* src = &map->entries[i];
        ori_map_set_value_for_key(out, map->key_kind == ORI_KEY_UNKNOWN ? ORI_KEY_INT : map->key_kind, src->key, src->value, src->value_size);
    }
    return out;
}
static inline void ori_map_free(ori_map_t* map) {
    if (!map) return;
    ori_map_clear(map);
    free(map->entries);
    free(map);
}
static inline ori_list_t ori_map_keys(ori_map_t* map) {
    size_t elem_size = map && map->key_kind == ORI_KEY_STRING ? sizeof(ori_string_t) : sizeof(int64_t);
    ori_list_t out = ori_list_new(elem_size);
    if (!map) return out;
    for (size_t i = 0; i < map->len; i++) {
        if (map->key_kind == ORI_KEY_STRING) {
            ori_list_push(&out, &map->entries[i].key.string_value);
        } else {
            ori_list_push(&out, &map->entries[i].key.int_value);
        }
    }
    return out;
}
static inline ori_list_t ori_map_values(ori_map_t* map) {
    ori_list_t out = ori_list_new(sizeof(int64_t));
    if (!map) return out;
    for (size_t i = 0; i < map->len; i++) {
        int64_t value = map->entries[i].value ? *((int64_t*)map->entries[i].value) : 0;
        ori_list_push(&out, &value);
    }
    return out;
}
static inline ori_list_t ori_map_entries(ori_map_t* map) {
    (void)map;
    return ori_list_new(sizeof(int64_t));
}
static inline ori_map_t* ori_map_from_entries(ori_list_t entries) {
    (void)entries;
    return ori_map_new();
}
static inline ori_list_t ori_list_map(ori_list_t l, void* fn_ptr, void* env) {
    ori_list_t out = ori_list_new(sizeof(int64_t));
    if (!fn_ptr) return out;
    int64_t (*f)(void*, int64_t) = (int64_t (*)(void*, int64_t))fn_ptr;
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        int64_t mapped = f(env, item);
        ori_list_push(&out, &mapped);
    }
    return out;
}
static inline ori_list_t ori_list_filter(ori_list_t l, void* fn_ptr, void* env) {
    ori_list_t out = ori_list_new(l.elem_size ? l.elem_size : sizeof(int64_t));
    if (!fn_ptr) return out;
    bool (*f)(void*, int64_t) = (bool (*)(void*, int64_t))fn_ptr;
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        if (f(env, item)) {
            ori_list_push(&out, &item);
        }
    }
    return out;
}
static inline ori_list_t ori_iter_flat_map(ori_list_t l, void* fn_ptr, void* env) {
    ori_list_t out = ori_list_new(sizeof(int64_t));
    if (!fn_ptr) return out;
    ori_list_t (*f)(void*, int64_t) = (ori_list_t (*)(void*, int64_t))fn_ptr;
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        ori_list_t inner = f(env, item);
        for (size_t j = 0; j < inner.len; j++) {
            int64_t mapped = *((int64_t*)ori_list_at(&inner, (int64_t)j));
            ori_list_push(&out, &mapped);
        }
    }
    return out;
}
static inline bool ori_iter_any(ori_list_t l, void* fn_ptr, void* env) {
    if (!fn_ptr) return false;
    bool (*f)(void*, int64_t) = (bool (*)(void*, int64_t))fn_ptr;
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        if (f(env, item)) return true;
    }
    return false;
}
static inline bool ori_iter_all(ori_list_t l, void* fn_ptr, void* env) {
    if (!fn_ptr) return false;
    bool (*f)(void*, int64_t) = (bool (*)(void*, int64_t))fn_ptr;
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        if (!f(env, item)) return false;
    }
    return true;
}
static inline int64_t ori_iter_count_where(ori_list_t l, void* fn_ptr, void* env) {
    if (!fn_ptr) return 0;
    bool (*f)(void*, int64_t) = (bool (*)(void*, int64_t))fn_ptr;
    int64_t count = 0;
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        if (f(env, item)) count++;
    }
    return count;
}
static inline ori_list_t ori_iter_take(ori_list_t l, int64_t n) {
    ori_list_t out = ori_list_new(l.elem_size ? l.elem_size : sizeof(int64_t));
    if (n <= 0) return out;
    size_t limit = (size_t)n < l.len ? (size_t)n : l.len;
    for (size_t i = 0; i < limit; i++) {
        ori_list_push(&out, ori_list_at(&l, (int64_t)i));
    }
    return out;
}
static inline ori_list_t ori_iter_skip(ori_list_t l, int64_t n) {
    ori_list_t out = ori_list_new(l.elem_size ? l.elem_size : sizeof(int64_t));
    size_t start = n <= 0 ? 0 : (size_t)n;
    if (start > l.len) start = l.len;
    for (size_t i = start; i < l.len; i++) {
        ori_list_push(&out, ori_list_at(&l, (int64_t)i));
    }
    return out;
}
static inline ori_list_t ori_iter_reverse(ori_list_t l) {
    ori_list_t out = ori_list_new(l.elem_size ? l.elem_size : sizeof(int64_t));
    for (size_t remaining = l.len; remaining > 0; remaining--) {
        ori_list_push(&out, ori_list_at(&l, (int64_t)(remaining - 1)));
    }
    return out;
}
static inline int64_t ori_iter_reduce(ori_list_t l, int64_t initial, void* fn_ptr, void* env) {
    if (!fn_ptr) return initial;
    int64_t (*f)(void*, int64_t, int64_t) = (int64_t (*)(void*, int64_t, int64_t))fn_ptr;
    int64_t acc = initial;
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        acc = f(env, acc, item);
    }
    return acc;
}
static inline ori_opt_i64_t ori_iter_find(ori_list_t l, void* fn_ptr, void* env) {
    if (!fn_ptr) return ((ori_opt_i64_t){ .has_value = false, .value = 0 });
    bool (*f)(void*, int64_t) = (bool (*)(void*, int64_t))fn_ptr;
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        if (f(env, item)) {
            return ((ori_opt_i64_t){ .has_value = true, .value = item });
        }
    }
    return ((ori_opt_i64_t){ .has_value = false, .value = 0 });
}
static inline ori_list_t ori_iter_sort(ori_list_t l) {
    ori_list_t out = ori_list_new(l.elem_size ? l.elem_size : sizeof(int64_t));
    for (size_t i = 0; i < l.len; i++) {
        ori_list_push(&out, ori_list_at(&l, (int64_t)i));
    }
    for (size_t i = 1; i < out.len; i++) {
        int64_t value = *((int64_t*)ori_list_at(&out, (int64_t)i));
        size_t j = i;
        while (j > 0) {
            int64_t prev = *((int64_t*)ori_list_at(&out, (int64_t)(j - 1)));
            if (prev <= value) break;
            *((int64_t*)ori_list_at(&out, (int64_t)j)) = prev;
            j--;
        }
        *((int64_t*)ori_list_at(&out, (int64_t)j)) = value;
    }
    return out;
}
static inline ori_list_t ori_iter_sort_string(ori_list_t l) {
    ori_list_t out = ori_list_new(sizeof(ori_string_t));
    for (size_t i = 0; i < l.len; i++) {
        ori_list_push(&out, ori_list_at(&l, (int64_t)i));
    }
    for (size_t i = 1; i < out.len; i++) {
        ori_string_t value = *((ori_string_t*)ori_list_at(&out, (int64_t)i));
        size_t j = i;
        while (j > 0) {
            ori_string_t prev = *((ori_string_t*)ori_list_at(&out, (int64_t)(j - 1)));
            size_t min_len = prev.len < value.len ? prev.len : value.len;
            int cmp = min_len == 0 ? 0 : memcmp(prev.data, value.data, min_len);
            if (cmp < 0 || (cmp == 0 && prev.len <= value.len)) break;
            *((ori_string_t*)ori_list_at(&out, (int64_t)j)) = prev;
            j--;
        }
        *((ori_string_t*)ori_list_at(&out, (int64_t)j)) = value;
    }
    return out;
}
static inline ori_list_t ori_iter_sort_by(ori_list_t l, void* fn_ptr, void* env) {
    ori_list_t out = ori_list_new(l.elem_size ? l.elem_size : sizeof(int64_t));
    for (size_t i = 0; i < l.len; i++) {
        ori_list_push(&out, ori_list_at(&l, (int64_t)i));
    }
    if (!fn_ptr) return out;
    int64_t (*compare)(void*, int64_t, int64_t) =
        (int64_t (*)(void*, int64_t, int64_t))fn_ptr;
    for (size_t i = 1; i < out.len; i++) {
        int64_t value = *((int64_t*)ori_list_at(&out, (int64_t)i));
        size_t j = i;
        while (j > 0) {
            int64_t prev = *((int64_t*)ori_list_at(&out, (int64_t)(j - 1)));
            if (compare(env, prev, value) <= 0) break;
            *((int64_t*)ori_list_at(&out, (int64_t)j)) = prev;
            j--;
        }
        *((int64_t*)ori_list_at(&out, (int64_t)j)) = value;
    }
    return out;
}
static inline ori_list_t ori_iter_unique(ori_list_t l) {
    ori_list_t out = ori_list_new(l.elem_size ? l.elem_size : sizeof(int64_t));
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        bool seen = false;
        for (size_t j = 0; j < out.len; j++) {
            if (*((int64_t*)ori_list_at(&out, (int64_t)j)) == item) {
                seen = true;
                break;
            }
        }
        if (!seen) ori_list_push(&out, &item);
    }
    return out;
}
static inline ori_list_t ori_iter_unique_string(ori_list_t l) {
    ori_list_t out = ori_list_new(sizeof(ori_string_t));
    for (size_t i = 0; i < l.len; i++) {
        ori_string_t item = *((ori_string_t*)ori_list_at(&l, (int64_t)i));
        bool seen = false;
        for (size_t j = 0; j < out.len; j++) {
            if (ori_string_eq(*((ori_string_t*)ori_list_at(&out, (int64_t)j)), item)) {
                seen = true;
                break;
            }
        }
        if (!seen) ori_list_push(&out, &item);
    }
    return out;
}
static inline ori_list_t ori_iter_zip(ori_list_t left, ori_list_t right) {
    typedef struct { int64_t _f0; int64_t _f1; } ori_zip_i64_pair_t;
    ori_list_t out = ori_list_new(sizeof(ori_zip_i64_pair_t));
    size_t limit = left.len < right.len ? left.len : right.len;
    for (size_t i = 0; i < limit; i++) {
        ori_zip_i64_pair_t pair = {
            ._f0 = *((int64_t*)ori_list_at(&left, (int64_t)i)),
            ._f1 = *((int64_t*)ori_list_at(&right, (int64_t)i)),
        };
        ori_list_push(&out, &pair);
    }
    return out;
}
static inline ori_tuple_list_i64_list_i64_t ori_iter_partition(ori_list_t l, void* fn_ptr, void* env) {
    ori_tuple_list_i64_list_i64_t out = {
        ._f0 = ori_list_new(l.elem_size ? l.elem_size : sizeof(int64_t)),
        ._f1 = ori_list_new(l.elem_size ? l.elem_size : sizeof(int64_t)),
    };
    if (!fn_ptr) return out;
    bool (*f)(void*, int64_t) = (bool (*)(void*, int64_t))fn_ptr;
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        if (f(env, item)) {
            ori_list_push(&out._f0, &item);
        } else {
            ori_list_push(&out._f1, &item);
        }
    }
    return out;
}
typedef struct {
    int64_t* keys;
    ori_list_t** values;
    size_t len;
    size_t cap;
} ori_group_i64_lists_t;
static inline ori_group_i64_lists_t* ori_group_i64_lists_new(void) {
    ori_group_i64_lists_t* groups = (ori_group_i64_lists_t*)calloc(1, sizeof(ori_group_i64_lists_t));
    if (!groups) abort();
    groups->cap = 4;
    groups->keys = (int64_t*)calloc(groups->cap, sizeof(int64_t));
    groups->values = (ori_list_t**)calloc(groups->cap, sizeof(ori_list_t*));
    if (!groups->keys || !groups->values) abort();
    return groups;
}
static inline ori_list_t* ori_group_i64_lists_bucket(ori_group_i64_lists_t* groups, int64_t key) {
    for (size_t i = 0; i < groups->len; i++) {
        if (groups->keys[i] == key) return groups->values[i];
    }
    if (groups->len >= groups->cap) {
        groups->cap *= 2;
        groups->keys = (int64_t*)realloc(groups->keys, groups->cap * sizeof(int64_t));
        groups->values = (ori_list_t**)realloc(groups->values, groups->cap * sizeof(ori_list_t*));
        if (!groups->keys || !groups->values) abort();
    }
    ori_list_t* bucket = (ori_list_t*)malloc(sizeof(ori_list_t));
    if (!bucket) abort();
    *bucket = ori_list_new(sizeof(int64_t));
    groups->keys[groups->len] = key;
    groups->values[groups->len] = bucket;
    groups->len++;
    return bucket;
}
static inline void* ori_iter_group_by(ori_list_t l, void* fn_ptr, void* env) {
    ori_group_i64_lists_t* groups = ori_group_i64_lists_new();
    if (!fn_ptr) return groups;
    int64_t (*key_fn)(void*, int64_t) = (int64_t (*)(void*, int64_t))fn_ptr;
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        int64_t key = key_fn(env, item);
        ori_list_t* bucket = ori_group_i64_lists_bucket(groups, key);
        ori_list_push(bucket, &item);
    }
    return groups;
}
static inline void* ori_iter_group_by_string(ori_list_t l, void* fn_ptr, void* env) {
    ori_group_i64_lists_t* groups = ori_group_i64_lists_new();
    if (!fn_ptr) return groups;
    int64_t (*key_fn)(void*, int64_t) = (int64_t (*)(void*, int64_t))fn_ptr;
    for (size_t i = 0; i < l.len; i++) {
        int64_t item = *((int64_t*)ori_list_at(&l, (int64_t)i));
        int64_t key = key_fn(env, item);
        ori_list_t* bucket = ori_group_i64_lists_bucket(groups, key);
        ori_list_push(bucket, &item);
    }
    return groups;
}
static inline ori_list_t ori_iter_flatten(ori_list_t nested) {
    ori_list_t out = ori_list_new(sizeof(int64_t));
    for (size_t i = 0; i < nested.len; i++) {
        ori_list_t* inner = (ori_list_t*)ori_list_at(&nested, (int64_t)i);
        if (!inner) continue;
        for (size_t j = 0; j < inner->len; j++) {
            int64_t item = *((int64_t*)ori_list_at(inner, (int64_t)j));
            ori_list_push(&out, &item);
        }
    }
    return out;
}

static inline ori_string_t ori_int_to_string(int64_t v) {
    char* buf = (char*)malloc(32);
    snprintf(buf, 32, "%" PRId64, v);
    return (ori_string_t){ .data = buf, .len = strlen(buf) };
}
static inline ori_string_t ori_float_to_string(double v) {
    int needed = snprintf(NULL, 0, "%.17g", v);
    if (needed < 0) abort();
    char* buf = (char*)malloc((size_t)needed + 1);
    if (!buf) abort();
    snprintf(buf, (size_t)needed + 1, "%.17g", v);
    return (ori_string_t){ .data = buf, .len = (size_t)needed };
}
static inline ori_string_t ori_bool_to_string(bool v) {
    return v ? ORI_STR("true") : ORI_STR("false");
}
static inline void ori_print_string(ori_string_t s) {
    fwrite(s.data, 1, s.len, stdout);
    putchar('\n');
}

static inline void ori_io_print(ori_string_t s) {
    ori_print_string(s);
}

static inline ori_string_t ori_to_string(int64_t v) {
    return ori_int_to_string(v);
}
static inline int64_t ori_to_int(int64_t v) { return v; }
static inline double ori_to_float(int64_t v) { return (double)v; }

static inline double ori_math_sqrt(double n) { return sqrt(n); }
static inline int64_t ori_math_abs(int64_t n) { return n < 0 ? -n : n; }
static inline double ori_math_abs_float(double n) { return n < 0.0 ? -n : n; }
static inline int64_t ori_math_min(int64_t a, int64_t b) { return a < b ? a : b; }
static inline double ori_math_min_float(double a, double b) { return a < b ? a : b; }
static inline int64_t ori_math_max(int64_t a, int64_t b) { return a > b ? a : b; }
static inline double ori_math_max_float(double a, double b) { return a > b ? a : b; }
static inline int64_t ori_math_clamp(int64_t value, int64_t min, int64_t max) {
    if (value < min) return min;
    if (value > max) return max;
    return value;
}
static inline double ori_math_pow(double base, double exp) { return pow(base, exp); }
static inline int64_t ori_math_floor(double n) { return (int64_t)floor(n); }
static inline int64_t ori_math_ceil(double n) { return (int64_t)ceil(n); }
static inline int64_t ori_math_round(double n) { return (int64_t)round(n); }
static inline double ori_math_log(double n) { return log(n); }
static inline double ori_math_log2(double n) { return log2(n); }
static inline double ori_math_sin(double n) { return sin(n); }
static inline double ori_math_cos(double n) { return cos(n); }
static inline double ori_math_tan(double n) { return tan(n); }
static inline bool ori_math_is_nan(double n) { return isnan(n); }
static inline bool ori_math_is_infinite(double n) { return isinf(n); }
static inline double ori_math_trunc(double n) { return trunc(n); }
static inline double ori_math_ln(double n) { return log(n); }
static inline double ori_math_exp(double n) { return exp(n); }
static inline double ori_math_asin(double n) { return asin(n); }
static inline double ori_math_acos(double n) { return acos(n); }
static inline double ori_math_atan(double n) { return atan(n); }
static inline double ori_math_atan2(double y, double x) { return atan2(y, x); }
static inline double ori_math_log10(double n) { return log10(n); }
static inline bool ori_math_is_finite(double n) { return isfinite(n); }
static inline int64_t ori_time_now(void) {
#if defined(_WIN32)
    FILETIME ft;
    ULARGE_INTEGER value;
    GetSystemTimeAsFileTime(&ft);
    value.LowPart = ft.dwLowDateTime;
    value.HighPart = ft.dwHighDateTime;
    return (int64_t)((value.QuadPart - 116444736000000000ULL) / 10000ULL);
#else
    struct timespec ts;
#if defined(TIME_UTC)
    if (timespec_get(&ts, TIME_UTC) == TIME_UTC) {
        return (int64_t)ts.tv_sec * 1000 + (int64_t)(ts.tv_nsec / 1000000);
    }
#endif
    return (int64_t)time(NULL) * 1000;
#endif
}
static inline void ori_time_sleep(int64_t millis) {
    if (millis <= 0) return;
#if defined(_WIN32)
    Sleep((DWORD)millis);
#else
    struct timespec req;
    req.tv_sec = (time_t)(millis / 1000);
    req.tv_nsec = (long)((millis % 1000) * 1000000);
    while (nanosleep(&req, &req) == -1) {}
#endif
}
static inline int64_t ori_time_duration_ms(int64_t start, int64_t end) { return end - start; }
static inline ori_string_t ori_format_number(double value, int64_t decimals) {
    if (decimals < 0) decimals = 0;
    if (decimals > 15) decimals = 15;
    int needed = snprintf(NULL, 0, "%.*f", (int)decimals, value);
    if (needed < 0) abort();
    char* buf = (char*)malloc((size_t)needed + 1);
    if (!buf) abort();
    snprintf(buf, (size_t)needed + 1, "%.*f", (int)decimals, value);
    return (ori_string_t){ .data = buf, .len = (size_t)needed };
}
static inline ori_string_t ori_format_percent(double value, int64_t decimals) {
    if (decimals < 0) decimals = 0;
    if (decimals > 15) decimals = 15;
    int needed = snprintf(NULL, 0, "%.*f%%", (int)decimals, value * 100.0);
    if (needed < 0) abort();
    char* buf = (char*)malloc((size_t)needed + 1);
    if (!buf) abort();
    snprintf(buf, (size_t)needed + 1, "%.*f%%", (int)decimals, value * 100.0);
    return (ori_string_t){ .data = buf, .len = (size_t)needed };
}
static inline ori_string_t ori_format_hex(int64_t value) {
    char tmp[32];
    int len = snprintf(tmp, sizeof(tmp), "%" PRIx64, (uint64_t)value);
    char* buf = (char*)malloc((size_t)len + 1);
    if (!buf) abort();
    memcpy(buf, tmp, (size_t)len + 1);
    return (ori_string_t){ .data = buf, .len = (size_t)len };
}
static inline ori_string_t ori_format_binary(int64_t value) {
    uint64_t v = (uint64_t)value;
    char tmp[65];
    int pos = 64;
    tmp[pos] = '\0';
    if (v == 0) {
        tmp[--pos] = '0';
    } else {
        while (v > 0 && pos > 0) {
            tmp[--pos] = (v & 1ULL) ? '1' : '0';
            v >>= 1;
        }
    }
    size_t len = strlen(&tmp[pos]);
    char* buf = (char*)malloc(len + 1);
    if (!buf) abort();
    memcpy(buf, &tmp[pos], len + 1);
    return (ori_string_t){ .data = buf, .len = len };
}
static inline bool ori_utc_parts(int64_t millis, int* year, int* month, int* day, int* hour, int* minute, int* second) {
    time_t secs = (time_t)(millis / 1000);
    struct tm tmv;
#if defined(_WIN32)
    if (gmtime_s(&tmv, &secs) != 0) return false;
#else
    if (gmtime_r(&secs, &tmv) == NULL) return false;
#endif
    *year = tmv.tm_year + 1900;
    *month = tmv.tm_mon + 1;
    *day = tmv.tm_mday;
    *hour = tmv.tm_hour;
    *minute = tmv.tm_min;
    *second = tmv.tm_sec;
    return true;
}
static inline ori_string_t ori_format_date(int64_t millis, ori_string_t style) {
    (void)style;
    int y, m, d, hh, mm, ss;
    if (!ori_utc_parts(millis, &y, &m, &d, &hh, &mm, &ss)) return ORI_STR("");
    char* buf = (char*)malloc(11);
    if (!buf) abort();
    snprintf(buf, 11, "%04d-%02d-%02d", y, m, d);
    return (ori_string_t){ .data = buf, .len = 10 };
}
static inline ori_string_t ori_format_datetime(int64_t millis, ori_string_t style, ori_string_t locale) {
    (void)style;
    (void)locale;
    int y, m, d, hh, mm, ss;
    if (!ori_utc_parts(millis, &y, &m, &d, &hh, &mm, &ss)) return ORI_STR("");
    char* buf = (char*)malloc(21);
    if (!buf) abort();
    snprintf(buf, 21, "%04d-%02d-%02dT%02d:%02d:%02dZ", y, m, d, hh, mm, ss);
    return (ori_string_t){ .data = buf, .len = 20 };
}
static inline ori_string_t ori_format_bytes_size(int64_t bytes, ori_string_t style) {
    bool binary = style.len == 6 && memcmp(style.data, "binary", 6) == 0;
    const char* decimal_units[] = {"B", "KB", "MB", "GB", "TB"};
    const char* binary_units[] = {"B", "KiB", "MiB", "GiB", "TiB"};
    const char** units = binary ? binary_units : decimal_units;
    double base = binary ? 1024.0 : 1000.0;
    double value = bytes < 0 ? -(double)bytes : (double)bytes;
    int unit = 0;
    while (value >= base && unit < 4) {
        value /= base;
        unit++;
    }
    int needed = unit == 0
        ? snprintf(NULL, 0, "%s%" PRId64 " %s", bytes < 0 ? "-" : "", (int64_t)value, units[unit])
        : snprintf(NULL, 0, "%s%.1f %s", bytes < 0 ? "-" : "", value, units[unit]);
    if (needed < 0) abort();
    char* buf = (char*)malloc((size_t)needed + 1);
    if (!buf) abort();
    if (unit == 0) {
        snprintf(buf, (size_t)needed + 1, "%s%" PRId64 " %s", bytes < 0 ? "-" : "", (int64_t)value, units[unit]);
    } else {
        snprintf(buf, (size_t)needed + 1, "%s%.1f %s", bytes < 0 ? "-" : "", value, units[unit]);
    }
    return (ori_string_t){ .data = buf, .len = (size_t)needed };
}

static inline char* ori_string_to_c_owned(ori_string_t s) {
    char* out = (char*)malloc(s.len + 1);
    if (!out) abort();
    if (s.len > 0) {
        memcpy(out, s.data, s.len);
    }
    out[s.len] = '\0';
    return out;
}

static int ori_runtime_argc = 0;
static char** ori_runtime_argv = NULL;

static inline void ori_os_set_args(int argc, char** argv) {
    ori_runtime_argc = argc;
    ori_runtime_argv = argv;
}

static inline ori_list_t ori_os_args(void) {
    ori_list_t out = ori_list_new(sizeof(ori_string_t));
    for (int i = 0; i < ori_runtime_argc; i++) {
        ori_string_t arg = ORI_STR_PTR(ori_runtime_argv && ori_runtime_argv[i] ? ori_runtime_argv[i] : "");
        ori_list_push(&out, &arg);
    }
    return out;
}

static inline ori_opt_str_t ori_os_env(ori_string_t name) {
    char* key = ori_string_to_c_owned(name);
    const char* value = getenv(key);
    free(key);
    if (!value) {
        return (ori_opt_str_t){ .has_value = false, .value = ORI_STR("") };
    }
    return (ori_opt_str_t){ .has_value = true, .value = ORI_STR_PTR(value) };
}

static inline void ori_os_exit(int64_t code) {
    exit((int)code);
}

static inline int64_t ori_os_pid(void) {
#if defined(_WIN32)
    return (int64_t)GetCurrentProcessId();
#else
    return (int64_t)getpid();
#endif
}

static inline ori_string_t ori_os_platform(void) {
#if defined(_WIN32)
    return ORI_STR("windows");
#elif defined(__APPLE__)
    return ORI_STR("macos");
#elif defined(__linux__)
    return ORI_STR("linux");
#else
    return ORI_STR("unknown");
#endif
}

static inline ori_string_t ori_os_arch(void) {
#if defined(__x86_64__) || defined(_M_X64)
    return ORI_STR("x86_64");
#elif defined(__aarch64__) || defined(_M_ARM64)
    return ORI_STR("aarch64");
#elif defined(__i386__) || defined(_M_IX86)
    return ORI_STR("x86");
#elif defined(__arm__) || defined(_M_ARM)
    return ORI_STR("arm");
#else
    return ORI_STR("unknown");
#endif
}

static inline void* ori_os_current_dir(void) {
    return NULL;
}

static inline void* ori_os_change_dir(ori_string_t path) {
    (void)path;
    return NULL;
}

static uint64_t ori_random_state = 0;

static inline uint64_t ori_random_next_u64(void) {
    if (ori_random_state == 0) {
        uintptr_t addr = (uintptr_t)&ori_random_state;
        uint64_t seed = (uint64_t)time(NULL) ^ (uint64_t)ori_os_pid() ^ (uint64_t)addr;
        ori_random_state = seed ? seed : UINT64_C(0x9e3779b97f4a7c15);
    }
    ori_random_state = ori_random_state * UINT64_C(6364136223846793005) + UINT64_C(1442695040888963407);
    return ori_random_state;
}

static inline int64_t ori_random_int(int64_t min, int64_t max) {
    if (max < min) {
        int64_t tmp = min;
        min = max;
        max = tmp;
    }
    uint64_t span = (uint64_t)max - (uint64_t)min + UINT64_C(1);
    uint64_t offset = span == 0 ? ori_random_next_u64() : ori_random_next_u64() % span;
    return (int64_t)((uint64_t)min + offset);
}

static inline double ori_random_unit_float(void) {
    return (double)(ori_random_next_u64() >> 11) * (1.0 / 9007199254740992.0);
}

static inline double ori_random_float(double min, double max) {
    if (max < min) {
        double tmp = min;
        min = max;
        max = tmp;
    }
    return min + (max - min) * ori_random_unit_float();
}

static inline bool ori_random_bool(void) {
    return (ori_random_next_u64() & UINT64_C(1)) != 0;
}

static inline ori_opt_i64_t ori_random_choice(ori_list_t items) {
    if (items.len == 0) {
        return (ori_opt_i64_t){ .has_value = false, .value = 0 };
    }
    size_t index = (size_t)(ori_random_next_u64() % items.len);
    int64_t value = *((int64_t*)ori_list_at(&items, (int64_t)index));
    return (ori_opt_i64_t){ .has_value = true, .value = value };
}

static inline ori_list_t ori_random_shuffle(ori_list_t items) {
    ori_list_t out = ori_list_new(items.elem_size ? items.elem_size : sizeof(int64_t));
    for (size_t i = 0; i < items.len; i++) {
        ori_list_push(&out, ori_list_at(&items, (int64_t)i));
    }
    for (size_t remaining = out.len; remaining > 1; remaining--) {
        size_t j = (size_t)(ori_random_next_u64() % remaining);
        int64_t a = *((int64_t*)ori_list_at(&out, (int64_t)(remaining - 1)));
        int64_t b = *((int64_t*)ori_list_at(&out, (int64_t)j));
        *((int64_t*)ori_list_at(&out, (int64_t)(remaining - 1))) = b;
        *((int64_t*)ori_list_at(&out, (int64_t)j)) = a;
    }
    return out;
}

static inline void ori_random_seed(int64_t seed) {
    ori_random_state = (uint64_t)seed;
}

static inline void ori_test_assert(bool condition, ori_string_t message) {
    if (!condition) {
        fprintf(stderr, "ori test assertion failed: %.*s\n", (int)message.len, message.data ? message.data : "");
        abort();
    }
}

static inline void ori_test_assert_eq(int64_t left, int64_t right) {
    if (left != right) {
        fprintf(stderr, "ori test assert_eq failed: %" PRId64 " != %" PRId64 "\n", left, right);
        abort();
    }
}

static inline void ori_test_assert_ne(int64_t left, int64_t right) {
    if (left == right) {
        fprintf(stderr, "ori test assert_ne failed: both values are %" PRId64 "\n", left);
        abort();
    }
}

static inline void ori_test_assert_eq_float(double left, double right) {
    if (left != right) {
        fprintf(stderr, "ori test assert_eq failed: %g != %g\n", left, right);
        abort();
    }
}

static inline void ori_test_assert_ne_float(double left, double right) {
    if (left == right) {
        fprintf(stderr, "ori test assert_ne failed: both values are %g\n", left);
        abort();
    }
}

static inline void ori_test_assert_eq_bool(bool left, bool right) {
    if (left != right) {
        fprintf(stderr, "ori test assert_eq failed: bool values differ\n");
        abort();
    }
}

static inline void ori_test_assert_ne_bool(bool left, bool right) {
    if (left == right) {
        fprintf(stderr, "ori test assert_ne failed: bool values are equal\n");
        abort();
    }
}

static inline void ori_test_assert_eq_string(ori_string_t left, ori_string_t right) {
    if (!ori_string_eq(left, right)) {
        fprintf(stderr, "ori test assert_eq failed: strings differ\n");
        abort();
    }
}

static inline void ori_test_assert_ne_string(ori_string_t left, ori_string_t right) {
    if (ori_string_eq(left, right)) {
        fprintf(stderr, "ori test assert_ne failed: strings are equal\n");
        abort();
    }
}

static inline void ori_test_fail(ori_string_t message) {
    fprintf(stderr, "ori test failure: %.*s\n", (int)message.len, message.data ? message.data : "");
    abort();
}

static inline void ori_test_skip(ori_string_t message) {
    (void)message;
    exit(77);
}

static inline int64_t ori_test_live_allocations(void) {
    /* Native runtime only; C debug backend has no ARC registry.
       Return 0 so test programs that call this on the C route do not fail. */
    return 0;
}

static inline int64_t ori_test_collect_cycles(void) {
    /* Native runtime only; C debug backend frees via explicit drop calls. */
    return 0;
}

static inline int64_t ori_test_assert_no_leaks(ori_string_t label) {
    (void)label;
    return 0;
}

/* Iterator signatures for opaque collections */
void* ori_deque_iterator_new(void* deque);
int64_t* ori_deque_iterator_next(void* iter);
void* ori_queue_iterator_new(void* queue);
int64_t* ori_queue_iterator_next(void* iter);
void* ori_stack_iterator_new(void* stack);
int64_t* ori_stack_iterator_next(void* iter);
void* ori_linked_list_iterator_new(void* list);
int64_t* ori_linked_list_iterator_next(void* iter);
void* ori_doubly_linked_list_iterator_new(void* list);
int64_t* ori_doubly_linked_list_iterator_next(void* iter);
void* ori_heap_iterator_new(void* heap);
int64_t* ori_heap_iterator_next(void* iter);
void* ori_graph_iterator_new(void* graph);
int64_t* ori_graph_iterator_next(void* iter);

typedef struct ori_arc_header {
    int64_t refcount;
    struct ori_arc_header* prev;
    struct ori_arc_header* next;
} ori_arc_header_t;
typedef struct ori_arc_edge {
    void* owner;
    void* child;
    struct ori_arc_edge* next;
} ori_arc_edge_t;
typedef struct {
    void* payload;
    ori_arc_header_t* header;
    int64_t trial_count;
    bool marked;
    bool collect;
} ori_arc_mark_t;
static ori_arc_header_t* ori_arc_head = NULL;
static ori_arc_edge_t* ori_arc_edges = NULL;
/* Amortized full collect: match native ORI_COOPERATIVE_COLLECT_THRESHOLD default. */
static unsigned long long ori_arc_alloc_counter = 0;
static unsigned long long ori_arc_last_collected = 0;
#ifndef ORI_COOPERATIVE_COLLECT_THRESHOLD
#define ORI_COOPERATIVE_COLLECT_THRESHOLD 256ull
#endif

static inline void ori_arc_release(void* ptr);
static inline long long ori_arc_collect_cycles(void);

static inline void ori_arc_maybe_collect_cycles(void) {
    if (ori_arc_alloc_counter - ori_arc_last_collected < ORI_COOPERATIVE_COLLECT_THRESHOLD) {
        return;
    }
    ori_arc_last_collected = ori_arc_alloc_counter;
    (void)ori_arc_collect_cycles();
}

static inline void* ori_alloc(size_t size, size_t align) {
    (void)align;
    ori_arc_header_t* header = (ori_arc_header_t*)calloc(1, sizeof(ori_arc_header_t) + size);
    if (!header) abort();
    header->refcount = 1;
    header->next = ori_arc_head;
    if (ori_arc_head) {
        ori_arc_head->prev = header;
    }
    ori_arc_head = header;
    ori_arc_alloc_counter++;
    return (void*)(header + 1);
}

static inline void* ori_arc_payload(ori_arc_header_t* header) {
    return (void*)(header + 1);
}

static inline ori_arc_header_t* ori_arc_find(void* ptr) {
    if (!ptr) return NULL;
    for (ori_arc_header_t* current = ori_arc_head; current; current = current->next) {
        if (ori_arc_payload(current) == ptr) {
            return current;
        }
    }
    return NULL;
}

static inline void ori_arc_unlink(ori_arc_header_t* header) {
    if (header->prev) {
        header->prev->next = header->next;
    } else {
        ori_arc_head = header->next;
    }
    if (header->next) {
        header->next->prev = header->prev;
    }
    header->prev = NULL;
    header->next = NULL;
}

static inline void ori_arc_remove_edges_referencing(void* ptr) {
    ori_arc_edge_t** current = &ori_arc_edges;
    while (*current) {
        ori_arc_edge_t* edge = *current;
        if (edge->owner == ptr || edge->child == ptr) {
            *current = edge->next;
            free(edge);
        } else {
            current = &edge->next;
        }
    }
}

static inline void ori_arc_release_owned_edges(void* owner) {
    ori_arc_edge_t** current = &ori_arc_edges;
    while (*current) {
        ori_arc_edge_t* edge = *current;
        if (edge->owner == owner) {
            void* child = edge->child;
            *current = edge->next;
            free(edge);
            ori_arc_release(child);
        } else {
            current = &edge->next;
        }
    }
}

static inline void ori_arc_free_object(ori_arc_header_t* header, bool release_owned_edges) {
    void* payload = ori_arc_payload(header);
    ori_arc_unlink(header);
    if (release_owned_edges) {
        ori_arc_release_owned_edges(payload);
    }
    ori_arc_remove_edges_referencing(payload);
    free(header);
}

static inline void ori_arc_retain(void* ptr) {
    ori_arc_header_t* header = ori_arc_find(ptr);
    if (header) {
        header->refcount++;
    }
}

static inline void ori_arc_release(void* ptr) {
    ori_arc_header_t* header = ori_arc_find(ptr);
    if (!header) {
        return;
    }
    header->refcount--;
    if (header->refcount <= 0) {
        ori_arc_free_object(header, true);
    }
}

static inline void ori_arc_register_edge(void* owner, void* child) {
    if (!owner || !child || owner == child) {
        return;
    }
    if (!ori_arc_find(owner) || !ori_arc_find(child)) {
        return;
    }
    for (ori_arc_edge_t* edge = ori_arc_edges; edge; edge = edge->next) {
        if (edge->owner == owner && edge->child == child) {
            return;
        }
    }
    ori_arc_edge_t* edge = (ori_arc_edge_t*)malloc(sizeof(ori_arc_edge_t));
    if (!edge) abort();
    edge->owner = owner;
    edge->child = child;
    edge->next = ori_arc_edges;
    ori_arc_edges = edge;
    ori_arc_retain(child);
}

static inline void ori_arc_unregister_edge(void* owner, void* child) {
    if (!owner || !child) {
        return;
    }
    ori_arc_edge_t** current = &ori_arc_edges;
    while (*current) {
        ori_arc_edge_t* edge = *current;
        if (edge->owner == owner && edge->child == child) {
            *current = edge->next;
            free(edge);
            ori_arc_release(child);
            return;
        }
        current = &edge->next;
    }
}

static inline void ori_arc_update_edge(void* owner, void* old_child, void* new_child) {
    if (old_child == new_child) {
        return;
    }
    ori_arc_unregister_edge(owner, old_child);
    ori_arc_register_edge(owner, new_child);
}

static inline long long ori_arc_index_of(ori_arc_mark_t* marks, size_t len, void* payload) {
    for (size_t i = 0; i < len; i++) {
        if (marks[i].payload == payload) {
            return (long long)i;
        }
    }
    return -1;
}

static inline void ori_arc_mark_reachable(size_t index, ori_arc_mark_t* marks, size_t len) {
    if (marks[index].marked) {
        return;
    }
    marks[index].marked = true;
    void* owner = marks[index].payload;
    for (ori_arc_edge_t* edge = ori_arc_edges; edge; edge = edge->next) {
        if (edge->owner != owner) {
            continue;
        }
        long long child_index = ori_arc_index_of(marks, len, edge->child);
        if (child_index >= 0) {
            ori_arc_mark_reachable((size_t)child_index, marks, len);
        }
    }
}

static inline void ori_arc_push_pending_release(
    void*** items,
    size_t* len,
    size_t* cap,
    void* value
) {
    if (*len >= *cap) {
        *cap = *cap ? *cap * 2 : 8;
        void** next = (void**)realloc(*items, *cap * sizeof(void*));
        if (!next) abort();
        *items = next;
    }
    (*items)[(*len)++] = value;
}

static inline long long ori_arc_collect_cycles(void) {
    size_t len = 0;
    for (ori_arc_header_t* header = ori_arc_head; header; header = header->next) {
        len++;
    }
    if (len == 0) {
        return 0;
    }

    ori_arc_mark_t* marks = (ori_arc_mark_t*)calloc(len, sizeof(ori_arc_mark_t));
    if (!marks) abort();
    size_t index = 0;
    for (ori_arc_header_t* header = ori_arc_head; header; header = header->next) {
        marks[index].payload = ori_arc_payload(header);
        marks[index].header = header;
        marks[index].trial_count = header->refcount;
        index++;
    }

    for (ori_arc_edge_t* edge = ori_arc_edges; edge; edge = edge->next) {
        long long owner_index = ori_arc_index_of(marks, len, edge->owner);
        if (owner_index < 0) {
            continue;
        }
        long long child_index = ori_arc_index_of(marks, len, edge->child);
        if (child_index >= 0) {
            marks[(size_t)child_index].trial_count--;
        }
    }

    for (size_t i = 0; i < len; i++) {
        if (marks[i].trial_count > 0) {
            ori_arc_mark_reachable(i, marks, len);
        }
    }

    long long collected = 0;
    for (size_t i = 0; i < len; i++) {
        if (!marks[i].marked) {
            marks[i].collect = true;
            collected++;
        }
    }
    if (collected == 0) {
        free(marks);
        return 0;
    }

    void** pending_release = NULL;
    size_t pending_len = 0;
    size_t pending_cap = 0;
    ori_arc_edge_t** current = &ori_arc_edges;
    while (*current) {
        ori_arc_edge_t* edge = *current;
        long long owner_index = ori_arc_index_of(marks, len, edge->owner);
        long long child_index = ori_arc_index_of(marks, len, edge->child);
        bool owner_collected = owner_index >= 0 && marks[(size_t)owner_index].collect;
        bool child_collected = child_index >= 0 && marks[(size_t)child_index].collect;
        if (owner_collected || child_collected) {
            *current = edge->next;
            if (owner_collected && !child_collected) {
                ori_arc_push_pending_release(
                    &pending_release,
                    &pending_len,
                    &pending_cap,
                    edge->child
                );
            }
            free(edge);
        } else {
            current = &edge->next;
        }
    }

    for (size_t i = 0; i < len; i++) {
        if (marks[i].collect) {
            ori_arc_unlink(marks[i].header);
            free(marks[i].header);
        }
    }

    for (size_t i = 0; i < pending_len; i++) {
        ori_arc_release(pending_release[i]);
    }
    free(pending_release);
    free(marks);
    return collected;
}

/* LANG-2: real C/debug bodies for string / convert / io / list helpers. */
static inline int64_t ori_string_len(ori_string_t s) { return (int64_t)s.len; }
static inline int64_t ori_len(ori_string_t s) { return (int64_t)s.len; }
static inline ori_string_t ori_string_dup_range(const char* data, size_t len) {
    char* out = (char*)malloc(len + 1);
    if (!out) abort();
    if (len > 0) memcpy(out, data, len);
    out[len] = '\0';
    return (ori_string_t){ .data = out, .len = len };
}
static inline ori_string_t ori_string_trim_start(ori_string_t s) {
    size_t i = 0;
    while (i < s.len && isspace((unsigned char)s.data[i])) i++;
    return ori_string_dup_range(s.data + i, s.len - i);
}
static inline ori_string_t ori_string_trim_end(ori_string_t s) {
    size_t n = s.len;
    while (n > 0 && isspace((unsigned char)s.data[n - 1])) n--;
    return ori_string_dup_range(s.data, n);
}
static inline ori_string_t ori_string_trim(ori_string_t s) {
    size_t start = 0;
    size_t end = s.len;
    while (start < end && isspace((unsigned char)s.data[start])) start++;
    while (end > start && isspace((unsigned char)s.data[end - 1])) end--;
    return ori_string_dup_range(s.data + start, end - start);
}
static inline ori_string_t ori_string_to_lower(ori_string_t s) {
    char* out = (char*)malloc(s.len + 1);
    if (!out) abort();
    for (size_t i = 0; i < s.len; i++) out[i] = (char)tolower((unsigned char)s.data[i]);
    out[s.len] = '\0';
    return (ori_string_t){ .data = out, .len = s.len };
}
static inline ori_string_t ori_string_to_upper(ori_string_t s) {
    char* out = (char*)malloc(s.len + 1);
    if (!out) abort();
    for (size_t i = 0; i < s.len; i++) out[i] = (char)toupper((unsigned char)s.data[i]);
    out[s.len] = '\0';
    return (ori_string_t){ .data = out, .len = s.len };
}
static inline ori_string_t ori_string_pad_left(ori_string_t s, int64_t width, ori_string_t pad) {
    if (width <= (int64_t)s.len || pad.len == 0) return ori_string_dup_range(s.data, s.len);
    size_t need = (size_t)width - s.len;
    char* out = (char*)malloc((size_t)width + 1);
    if (!out) abort();
    for (size_t i = 0; i < need; i++) out[i] = pad.data[i % pad.len];
    if (s.len > 0) memcpy(out + need, s.data, s.len);
    out[width] = '\0';
    return (ori_string_t){ .data = out, .len = (size_t)width };
}
static inline ori_string_t ori_string_pad_right(ori_string_t s, int64_t width, ori_string_t pad) {
    if (width <= (int64_t)s.len || pad.len == 0) return ori_string_dup_range(s.data, s.len);
    size_t need = (size_t)width - s.len;
    char* out = (char*)malloc((size_t)width + 1);
    if (!out) abort();
    if (s.len > 0) memcpy(out, s.data, s.len);
    for (size_t i = 0; i < need; i++) out[s.len + i] = pad.data[i % pad.len];
    out[width] = '\0';
    return (ori_string_t){ .data = out, .len = (size_t)width };
}
static inline bool ori_string_contains(ori_string_t s, ori_string_t sub) {
    if (sub.len == 0) return true;
    if (sub.len > s.len) return false;
    for (size_t i = 0; i + sub.len <= s.len; i++) {
        if (memcmp(s.data + i, sub.data, sub.len) == 0) return true;
    }
    return false;
}
static inline bool ori_string_starts_with(ori_string_t s, ori_string_t sub) {
    if (sub.len > s.len) return false;
    return sub.len == 0 || memcmp(s.data, sub.data, sub.len) == 0;
}
static inline bool ori_string_ends_with(ori_string_t s, ori_string_t sub) {
    if (sub.len > s.len) return false;
    return sub.len == 0 || memcmp(s.data + (s.len - sub.len), sub.data, sub.len) == 0;
}
static inline int64_t ori_string_index_of(ori_string_t s, ori_string_t sub) {
    if (sub.len == 0) return 0;
    if (sub.len > s.len) return -1;
    for (size_t i = 0; i + sub.len <= s.len; i++) {
        if (memcmp(s.data + i, sub.data, sub.len) == 0) return (int64_t)i;
    }
    return -1;
}
static inline ori_string_t ori_string_replace(ori_string_t s, ori_string_t from, ori_string_t to) {
    if (from.len == 0) return ori_string_dup_range(s.data, s.len);
    size_t count = 0;
    for (size_t i = 0; i + from.len <= s.len; ) {
        if (memcmp(s.data + i, from.data, from.len) == 0) {
            count++;
            i += from.len;
        } else {
            i++;
        }
    }
    size_t out_len = s.len + count * to.len - count * from.len;
    char* out = (char*)malloc(out_len + 1);
    if (!out) abort();
    size_t o = 0;
    for (size_t i = 0; i < s.len; ) {
        if (i + from.len <= s.len && memcmp(s.data + i, from.data, from.len) == 0) {
            if (to.len > 0) memcpy(out + o, to.data, to.len);
            o += to.len;
            i += from.len;
        } else {
            out[o++] = s.data[i++];
        }
    }
    out[out_len] = '\0';
    return (ori_string_t){ .data = out, .len = out_len };
}
static inline ori_list_t ori_string_split(ori_string_t s, ori_string_t delimiter) {
    ori_list_t out = ori_list_new(sizeof(ori_string_t));
    if (delimiter.len == 0) {
        ori_string_t whole = ori_string_dup_range(s.data, s.len);
        ori_list_push(&out, &whole);
        return out;
    }
    size_t start = 0;
    for (size_t i = 0; i + delimiter.len <= s.len; ) {
        if (memcmp(s.data + i, delimiter.data, delimiter.len) == 0) {
            ori_string_t part = ori_string_dup_range(s.data + start, i - start);
            ori_list_push(&out, &part);
            i += delimiter.len;
            start = i;
        } else {
            i++;
        }
    }
    ori_string_t tail = ori_string_dup_range(s.data + start, s.len - start);
    ori_list_push(&out, &tail);
    return out;
}
static inline ori_list_t ori_string_chars(ori_string_t s) {
    ori_list_t out = ori_list_new(sizeof(ori_string_t));
    for (size_t i = 0; i < s.len; i++) {
        ori_string_t ch = ori_string_dup_range(s.data + i, 1);
        ori_list_push(&out, &ch);
    }
    return out;
}
static inline ori_string_t ori_string_join(ori_list_t list, ori_string_t sep) {
    size_t total = 0;
    for (size_t i = 0; i < list.len; i++) {
        ori_string_t* part = (ori_string_t*)ori_list_at(&list, (int64_t)i);
        total += part->len;
        if (i + 1 < list.len) total += sep.len;
    }
    char* out = (char*)malloc(total + 1);
    if (!out) abort();
    size_t o = 0;
    for (size_t i = 0; i < list.len; i++) {
        ori_string_t* part = (ori_string_t*)ori_list_at(&list, (int64_t)i);
        if (part->len > 0) {
            memcpy(out + o, part->data, part->len);
            o += part->len;
        }
        if (i + 1 < list.len && sep.len > 0) {
            memcpy(out + o, sep.data, sep.len);
            o += sep.len;
        }
    }
    out[total] = '\0';
    return (ori_string_t){ .data = out, .len = total };
}
static inline ori_string_t ori_string_repeat(ori_string_t s, int64_t count) {
    if (count <= 0 || s.len == 0) return ori_string_dup_range("", 0);
    size_t n = (size_t)count;
    size_t total = s.len * n;
    char* out = (char*)malloc(total + 1);
    if (!out) abort();
    for (size_t i = 0; i < n; i++) memcpy(out + i * s.len, s.data, s.len);
    out[total] = '\0';
    return (ori_string_t){ .data = out, .len = total };
}
static inline int64_t ori_list_len(ori_list_t list) { return (int64_t)list.len; }
static inline ori_opt_i64_t ori_list_try_get(ori_list_t list, int64_t index) {
    if (index < 0 || index >= (int64_t)list.len) {
        return (ori_opt_i64_t){ .has_value = false, .value = 0 };
    }
    if (list.elem_size == sizeof(int64_t)) {
        return (ori_opt_i64_t){ .has_value = true, .value = *((int64_t*)ori_list_at(&list, index)) };
    }
    return (ori_opt_i64_t){ .has_value = true, .value = 0 };
}
static inline void* ori_list_get(ori_list_t list, int64_t index) {
    return ori_list_at(&list, index);
}
static inline void ori_io_eprint(ori_string_t s) {
    fwrite(s.data, 1, s.len, stderr);
    fputc('\n', stderr);
    fflush(stderr);
}
static inline ori_opt_str_t ori_io_read_line(void) {
#if defined(_WIN32)
    char buf[4096];
    if (!fgets(buf, (int)sizeof(buf), stdin)) {
        return (ori_opt_str_t){ .has_value = false, .value = ORI_STR("") };
    }
    size_t n = strlen(buf);
    while (n > 0 && (buf[n - 1] == '\n' || buf[n - 1] == '\r')) n--;
    return (ori_opt_str_t){ .has_value = true, .value = ori_string_dup_range(buf, n) };
#else
    char* line = NULL;
    size_t cap = 0;
    ssize_t n = getline(&line, &cap, stdin);
    if (n < 0) {
        free(line);
        return (ori_opt_str_t){ .has_value = false, .value = ORI_STR("") };
    }
    while (n > 0 && (line[n - 1] == '\n' || line[n - 1] == '\r')) n--;
    ori_string_t value = ori_string_dup_range(line, (size_t)n);
    free(line);
    return (ori_opt_str_t){ .has_value = true, .value = value };
#endif
}
static inline ori_opt_i64_t ori_string_to_int(ori_string_t s) {
    char* owned = ori_string_to_c_owned(s);
    char* end = NULL;
    long long v = strtoll(owned, &end, 10);
    bool ok = end != owned && end && *end == '\0';
    free(owned);
    return (ori_opt_i64_t){ .has_value = ok, .value = ok ? (int64_t)v : 0 };
}
/* ori_string_to_float is provided after codegen emits ori_opt_f64_t (optional[float]). */
static inline int64_t ori_bytes_len(void* ptr) { (void)ptr; return 0; }
static inline uint8_t ori_bytes_get(void* ptr, int64_t index) { (void)ptr; (void)index; return 0; }
static inline void* ori_bytes_from_list(ori_list_t list) { (void)list; return NULL; }
static inline void ori_io_close_input(void* ptr) { (void)ptr; }
static inline void ori_io_close_output(void* ptr) { (void)ptr; }
static inline uint8_t* ori_bytes_concat(uint8_t* left, uint8_t* right) { (void)left; (void)right; return NULL; }
static inline ori_list_t ori_bytes_to_list(uint8_t* bytes) { (void)bytes; return (ori_list_t){0}; }
static inline uint8_t* ori_string_to_bytes(ori_string_t s) {
    uint8_t* out = (uint8_t*)malloc(s.len + 1);
    if (!out) abort();
    if (s.len > 0) memcpy(out, s.data, s.len);
    out[s.len] = 0;
    return out;
}
"#;

// ── Codegen context ───────────────────────────────────────────────────────────

pub struct CCodegen {
    out: String,
    indent: usize,
    tmp_ctr: usize,
    errors: Vec<String>,
    /// Set of top-level Ori function names (unmangled). Used to prefix calls with `ORI__`.
    func_names: HashSet<smol_str::SmolStr>,
    type_names: HashMap<DefId, smol_str::SmolStr>,
    struct_fields: HashMap<DefId, Vec<(smol_str::SmolStr, Ty)>>,
    trait_layouts: HashMap<DefId, HirTrait>,
    trait_impls: HashMap<(DefId, DefId), HirTraitImpl>,
    using_stack: Vec<(String, Ty)>,
    managed_stack: Vec<(String, Ty)>,
    loop_stack: Vec<(usize, usize)>,
    current_return_ty: Option<Ty>,
}

impl CCodegen {
    pub fn new() -> Self {
        Self {
            out: String::new(),
            indent: 0,
            tmp_ctr: 0,
            errors: Vec::new(),
            func_names: Default::default(),
            type_names: Default::default(),
            struct_fields: Default::default(),
            trait_layouts: Default::default(),
            trait_impls: Default::default(),
            using_stack: Default::default(),
            managed_stack: Default::default(),
            loop_stack: Default::default(),
            current_return_ty: None,
        }
    }

    fn fresh_tmp(&mut self) -> String {
        self.tmp_ctr += 1;
        format!("_ori_tmp{}", self.tmp_ctr)
    }

    fn type_supports_equality(&self, ty: &Ty, visiting: &mut Vec<DefId>) -> bool {
        match ty {
            Ty::Void
            | Ty::Never
            | Ty::Bool
            | Ty::Int
            | Ty::Float
            | Ty::Float32
            | Ty::Float64
            | Ty::U8
            | Ty::U16
            | Ty::U32
            | Ty::U64
            | Ty::Int8
            | Ty::Int16
            | Ty::Int32
            | Ty::Int64
            | Ty::String
            | Ty::Bytes
            | Ty::Any(_) => true,
            Ty::Optional(inner) => self.type_supports_equality(inner, visiting),
            Ty::Result(ok, err) => {
                self.type_supports_equality(ok, visiting)
                    && self.type_supports_equality(err, visiting)
            }
            Ty::Tuple(elements) => elements
                .iter()
                .all(|e| self.type_supports_equality(e, visiting)),
            Ty::List(inner) | Ty::Set(inner) => self.type_supports_equality(inner, visiting),
            Ty::Map(k, v) => {
                self.type_supports_equality(k, visiting) && self.type_supports_equality(v, visiting)
            }
            Ty::Named(def_id, args) => {
                if visiting.contains(def_id) {
                    return true;
                }
                visiting.push(*def_id);
                let ok = if let Some(fields) = self.struct_fields.get(def_id) {
                    fields.iter().all(|(_, f_ty)| {
                        let concrete = substitute_ty_params(f_ty, args);
                        self.type_supports_equality(&concrete, visiting)
                    })
                } else {
                    false
                };
                visiting.pop();
                ok
            }
            Ty::Opaque { kind, args } if kind.is_list_backed_collection() => {
                if let Some(elem_ty) = args.first() {
                    self.type_supports_equality(elem_ty, visiting)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn emit_indent(&mut self) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
    }

    fn line(&mut self, s: &str) {
        self.emit_indent();
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn push(&mut self) {
        self.indent += 1;
    }
    fn pop(&mut self) {
        self.indent -= 1;
    }

    fn push_codegen_error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }

    fn unsupported_expr(&mut self, message: impl Into<String>) -> String {
        self.push_codegen_error(message);
        "0".into()
    }

    pub fn generate(mut self, module: &HirModule) -> Result<String, String> {
        // Collect function names for call-site mangling.
        // We include both Ori-defined functions AND extern C functions so that
        // the Call emitter can distinguish direct calls from closure variable calls.
        for f in &module.funcs {
            self.func_names.insert(f.name.clone());
        }
        for ext in &module.externs {
            if let HirExtern::Func { name, .. } = ext {
                self.func_names.insert(name.clone());
            }
        }
        for s in &module.structs {
            self.type_names.insert(s.def_id, s.name.clone());
            self.struct_fields.insert(
                s.def_id,
                s.fields
                    .iter()
                    .map(|field| (field.name.clone(), field.ty.clone()))
                    .collect(),
            );
        }
        for e in &module.enums {
            self.type_names.insert(e.def_id, e.name.clone());
        }
        for t in &module.traits {
            self.trait_layouts.insert(t.def_id, t.clone());
        }
        for imp in &module.trait_impls {
            self.trait_impls
                .insert((imp.trait_def_id, imp.type_def_id), imp.clone());
        }

        // Preamble
        self.out.push_str(ORI_RUNTIME_H);
        self.out.push('\n');

        // Forward declarations for all structs
        let mut forwarded_structs = HashSet::new();
        for s in &module.structs {
            if !forwarded_structs.insert(s.def_id) {
                continue;
            }
            let name = def_c_name(s.def_id);
            self.line(&format!("typedef struct {} {};", name, name));
        }
        // Forward declarations and empty structs for traits used by default methods.
        let mut emitted_trait_types = HashSet::new();
        for t in &module.traits {
            if !t
                .methods
                .iter()
                .any(|method| method.default_func_name.is_some())
            {
                continue;
            }
            if !emitted_trait_types.insert(t.def_id) {
                continue;
            }
            let name = def_c_name(t.def_id);
            self.line(&format!(
                "typedef struct {} {{ uint8_t _empty; }} {};",
                name, name
            ));
        }
        if !module.structs.is_empty() || !module.traits.is_empty() {
            self.out.push('\n');
        }

        let abi_types = collect_abi_types(module);
        for ty in &abi_types {
            self.emit_abi_type_def(&ty);
        }
        if !abi_types.is_empty() {
            self.out.push('\n');
        }

        // Struct definitions
        let mut emitted_structs = HashSet::new();
        for s in &module.structs {
            if !emitted_structs.insert(s.def_id) {
                continue;
            }
            self.emit_struct(s);
        }

        // Emit static inline equality helper functions for structs
        let mut emitted_eq_helpers = std::collections::HashSet::new();
        for s in &module.structs {
            if !emitted_eq_helpers.insert(s.def_id) {
                continue;
            }
            let name = format!("__eq_helper_struct_{}", s.def_id.0);
            if self.type_supports_equality(&Ty::Named(s.def_id, Vec::new()), &mut Vec::new()) {
                let ty_c = def_c_name(s.def_id);
                let expr = self.struct_equality_to_c(
                    "(*left)".to_string(),
                    "(*right)".to_string(),
                    s.def_id,
                    &[],
                );
                self.line(&format!(
                    "static inline bool {}(void* left_ptr, void* right_ptr) {{\n    if (!left_ptr || !right_ptr) return left_ptr == right_ptr;\n    const {}* left = (const {}*)left_ptr;\n    const {}* right = (const {}*)right_ptr;\n    return {};\n}}",
                    name, ty_c, ty_c, ty_c, ty_c, expr
                ));
            } else {
                self.line(&format!(
                    "static inline bool {}(void* left_ptr, void* right_ptr) {{\n    return false;\n}}",
                    name
                ));
            }
        }

        // Enum definitions (tagged unions)
        for e in &module.enums {
            self.emit_enum(e);
        }

        // Extern declarations
        for ext in &module.externs {
            match ext {
                HirExtern::Func {
                    name,
                    params,
                    return_ty,
                    ..
                } => {
                    let ret_s = ty_to_c(return_ty);
                    let params_s: Vec<String> = params
                        .iter()
                        .map(|p| format!("{} {}", ty_to_c(&p.ty), mangle(&p.name)))
                        .collect();
                    let params_str = if params_s.is_empty() {
                        "void".to_string()
                    } else {
                        params_s.join(", ")
                    };
                    self.line(&format!(
                        "extern {} {}({});",
                        ret_s,
                        mangle(name),
                        params_str
                    ));
                }
                HirExtern::Var { name, ty, .. } => {
                    self.line(&format!("extern {} {};", ty_to_c(ty), mangle(name)));
                }
            }
        }
        if !module.externs.is_empty() {
            self.out.push('\n');
        }

        // Forward declarations for functions
        for f in &module.funcs {
            if !f.closure_captures.is_empty() {
                self.out.push_str(&format!("typedef struct {{\n"));
                for cap in &f.closure_captures {
                    self.out.push_str(&format!(
                        "    {} {};\n",
                        ty_to_c(&cap.ty),
                        mangle(&cap.name)
                    ));
                }
                self.out
                    .push_str(&format!("}} {}_env_t;\n", Self::func_c_name(&f.name)));
            }
            let sig = self.func_signature(f);
            self.out.push_str(&sig);
            self.out.push_str(";\n");
        }
        if !module.funcs.is_empty() {
            self.out.push('\n');
        }

        // Constant/global variable definitions
        for c in &module.consts {
            let ty_s = ty_to_c(&c.ty);
            let val_s = self.expr_to_c(&c.value);
            if c.mutable {
                self.line(&format!("static {} {} = {};", ty_s, mangle(&c.name), val_s));
            } else {
                self.line(&format!(
                    "static const {} {} = {};",
                    ty_s,
                    mangle(&c.name),
                    val_s
                ));
            }
        }
        if !module.consts.is_empty() {
            self.out.push('\n');
        }

        // Function definitions
        for f in &module.funcs {
            if f.is_async || matches!(f.return_ty, Ty::Future(_)) {
                self.push_codegen_error(
                    "C backend does not support async functions yet; use the native backend",
                );
            }
            self.emit_func(f);
        }

        // Entry point: if there is a `main` func with no params, wrap it in C main
        if let Some(main_fn) = module.funcs.iter().find(|f| is_entry_main(module, f)) {
            self.out.push_str("int main(int argc, char** argv) {\n");
            self.out.push_str("    ori_os_set_args(argc, argv);\n");
            self.out
                .push_str(&format!("    {}();\n", Self::func_c_name(&main_fn.name)));
            self.out.push_str("    return 0;\n}\n");
        }

        if self.errors.is_empty() {
            Ok(self.out)
        } else {
            Err(format!(
                "C backend codegen failed:\n- {}",
                self.errors.join("\n- ")
            ))
        }
    }

    // ── Struct ────────────────────────────────────────────────────────────────

    fn emit_struct(&mut self, s: &HirStruct) {
        self.line(&format!("struct {} {{", def_c_name(s.def_id)));
        self.push();
        for f in &s.fields {
            self.line(&format!("{} {};", ty_to_c(&f.ty), mangle(&f.name)));
        }
        self.pop();
        self.line("};");
        self.out.push('\n');
    }

    // ── Enum (tagged union) ───────────────────────────────────────────────────

    fn emit_enum(&mut self, e: &HirEnum) {
        let name = def_c_name(e.def_id);
        // Discriminant enum
        self.line(&format!("typedef enum {{"));
        self.push();
        for v in &e.variants {
            self.line(&format!("{}__{},", name, mangle(&v.name)));
        }
        self.pop();
        self.line(&format!("}} {}_tag_t;", name));
        self.out.push('\n');

        // Payload union + outer struct
        self.line(&format!("typedef struct {} {{", name));
        self.push();
        self.line(&format!("{}_tag_t tag;", name));
        self.line("union {");
        self.push();
        for v in &e.variants {
            if !v.fields.is_empty() {
                self.line("struct {");
                self.push();
                for f in &v.fields {
                    self.line(&format!("{} {};", ty_to_c(&f.ty), mangle(&f.name)));
                }
                self.pop();
                self.line(&format!("}} {};", mangle(&v.name)));
            }
        }
        self.pop();
        self.line("} payload;");
        self.pop();
        self.line(&format!("}} {};", name));
        self.out.push('\n');
    }

    fn emit_abi_type_def(&mut self, ty: &Ty) {
        match ty {
            Ty::Optional(inner) => {
                if matches!(inner.as_ref(), Ty::Int | Ty::String) {
                    return;
                }
                self.line(&format!(
                    "typedef struct {{ bool has_value; {} value; }} {};",
                    abi_value_c_type(inner),
                    ty_to_c(ty),
                ));
            }
            Ty::Result(ok, err) => {
                self.line(&format!("typedef struct {} {{", ty_to_c(ty)));
                self.push();
                self.line("bool is_ok;");
                self.line("union {");
                self.push();
                self.line(&format!("{} ok;", abi_value_c_type(ok)));
                self.line(&format!("{} err;", abi_value_c_type(err)));
                self.pop();
                self.line("} value;");
                self.pop();
                self.line(&format!("}} {};", ty_to_c(ty)));
            }
            Ty::Tuple(elems) => {
                if ty_to_c(ty) == "ori_tuple_list_i64_list_i64_t" {
                    return;
                }
                self.line("typedef struct {");
                self.push();
                for (index, elem_ty) in elems.iter().enumerate() {
                    self.line(&format!("{} _f{};", ty_to_c(elem_ty), index));
                }
                self.pop();
                self.line(&format!("}} {};", ty_to_c(ty)));
            }
            Ty::Lazy(inner) => {
                self.line(&format!("typedef struct {} {{", lazy_c_name(inner)));
                self.push();
                self.line("ori_closure_t* thunk;");
                self.line("bool forced;");
                self.line(&format!("{} value;", ty_to_c(inner)));
                self.pop();
                self.line(&format!("}} {};", lazy_c_name(inner)));
            }
            _ => {}
        }
    }

    // ── Functions ─────────────────────────────────────────────────────────────

    fn func_c_name(name: &str) -> String {
        format!("ORI__{}", mangle(name))
    }

    fn func_signature(&self, f: &HirFunc) -> String {
        let ret = ty_to_c(&f.return_ty);
        let name = Self::func_c_name(&f.name);
        let mut params: Vec<String> = Vec::new();
        // Closure functions receive a `void* __env` as their first hidden argument.
        if !f.closure_captures.is_empty() {
            params.push("void* __env".to_string());
        }
        params.extend(
            f.params
                .iter()
                .filter(|p| f.closure_captures.is_empty() || p.name.as_str() != "__env")
                .map(|p| format!("{} {}", ty_to_c(&p.ty), mangle(&p.name))),
        );
        let param_str = if params.is_empty() {
            "void".into()
        } else {
            params.join(", ")
        };
        format!("{} {}({})", ret, name, param_str)
    }

    fn emit_func(&mut self, f: &HirFunc) {
        let sig = self.func_signature(f);
        self.out.push_str(&sig);
        self.out.push_str(
            " {
",
        );
        self.push();
        // Unpack captured environment for closures.
        if !f.closure_captures.is_empty() {
            let env_name = format!("{}_env_t", Self::func_c_name(&f.name));
            self.line(&format!("{}* _env = ({}*)__env;", env_name, env_name));
            for cap in &f.closure_captures {
                self.line(&format!(
                    "{} {} = _env->{};",
                    ty_to_c(&cap.ty),
                    mangle(&cap.name),
                    mangle(&cap.name)
                ));
            }
        }
        // Value contract checks for parameters.
        for param in &f.params {
            if let Some(contract) = &param.contract {
                let p_c = mangle(&param.name);
                let p_ty = ty_to_c(&param.ty);
                let cond_s = self.expr_to_c(contract);
                let param_name = param.name.as_str();
                self.line(&format!(
                    "{{ {p_ty} it = {p_c}; (void)it; if (!({cond_s})) {{ fprintf(stderr, \"value contract violated for parameter '{param_name}'\n\"); abort(); }} }}"
                ));
            }
        }
        let previous_return_ty = self.current_return_ty.replace(f.return_ty.clone());
        self.emit_block(&f.body.stmts);
        self.current_return_ty = previous_return_ty;
        self.pop();
        self.line("}");
        self.out.push('\n');
    }

    // ── Statements ────────────────────────────────────────────────────────────

    fn emit_block(&mut self, stmts: &[HirStmt]) {
        let cleanup_start = self.using_stack.len();
        let managed_cleanup_start = self.managed_stack.len();
        for stmt in stmts {
            self.emit_stmt(stmt);
        }
        self.emit_cleanups_from(cleanup_start, managed_cleanup_start);
        self.using_stack.truncate(cleanup_start);
        self.managed_stack.truncate(managed_cleanup_start);
    }

    fn emit_cleanups_from(&mut self, using_start: usize, managed_start: usize) {
        for statement in self.cleanup_statements_from(using_start, managed_start) {
            self.line(&statement);
        }
    }

    fn cleanup_statements_from(&self, using_start: usize, managed_start: usize) -> Vec<String> {
        let mut statements = Vec::new();
        for (name, ty) in self.using_stack[using_start..].iter().rev() {
            if let Ty::Named(def_id, _) = ty {
                if let Some(type_name) = self.type_names.get(def_id) {
                    let dispose_fn = format!("ORI__{}_dispose", mangle(type_name));
                    statements.push(format!("{}({});", dispose_fn, mangle(name)));
                }
            }
        }
        for (name, ty) in self.managed_stack[managed_start..].iter().rev() {
            if let Some(access) = c_arc_access(&mangle(name), ty) {
                statements.push(format!("ori_arc_release({});", access));
            }
        }
        if managed_start == 0 {
            // Amortized gate (matches native ori_arc_maybe_collect_cycles).
            statements.push("ori_arc_maybe_collect_cycles();".to_string());
        }
        statements
    }

    fn emit_stmt(&mut self, stmt: &HirStmt) {
        match stmt {
            HirStmt::Let {
                name, ty, value, ..
            } => {
                let val_s = self.expr_to_c_for_expected(value, ty);
                self.line(&format!("{} {} = {};", ty_to_c(ty), mangle(name), val_s));
                if let Some(access) = c_arc_access(&mangle(name), ty) {
                    self.line(&format!("ori_arc_retain({});", access));
                    self.managed_stack.push((name.to_string(), ty.clone()));
                }
            }
            HirStmt::Assign { lvalue, value, .. } => {
                let lv = self.lvalue_to_c(lvalue);
                // Currently, we don't have expected_ty easily available for lvalue without resolving it,
                // but we can try to get it from value.ty since semantic analysis ensures it's correct.
                // However, value.ty might not be the trait type. We'll use value.ty for expected type.
                let val_s = self.expr_to_c_for_expected(value, &value.ty);
                if let HirLValue::Var(name) = lvalue {
                    if let Some(ty) = self.managed_local_ty(name).cloned() {
                        if let Some(access) = c_arc_access(&lv, &ty) {
                            let old_tmp = self.fresh_tmp();
                            self.line(&format!("void* {} = {};", old_tmp, access));
                            self.line(&format!("{} = {};", lv, val_s));
                            if let Some(new_access) = c_arc_access(&lv, &ty) {
                                self.line(&format!("ori_arc_retain({});", new_access));
                            }
                            self.line(&format!("ori_arc_release({});", old_tmp));
                            return;
                        }
                    }
                }
                self.line(&format!("{} = {};", lv, val_s));
            }
            HirStmt::Return(val, _) => match val {
                Some(e) => {
                    let return_ty = self
                        .current_return_ty
                        .clone()
                        .unwrap_or_else(|| e.ty.clone());
                    let val_s = self.expr_to_c_for_expected(e, &return_ty);
                    if c_arc_access("__ori_return_value", &return_ty).is_some() {
                        let ret_tmp = self.fresh_tmp();
                        self.line(&format!("{} {} = {};", ty_to_c(&return_ty), ret_tmp, val_s));
                        if let Some(access) = c_arc_access(&ret_tmp, &return_ty) {
                            self.line(&format!("ori_arc_retain({});", access));
                        }
                        self.emit_cleanups_from(0, 0);
                        self.line(&format!("return {};", ret_tmp));
                    } else {
                        self.emit_cleanups_from(0, 0);
                        self.line(&format!("return {};", val_s));
                    }
                }
                None => {
                    self.emit_cleanups_from(0, 0);
                    self.line("return;");
                }
            },
            HirStmt::Break(_) => {
                if let Some(loop_start) = self.loop_stack.last().copied() {
                    self.emit_cleanups_from(loop_start.0, loop_start.1);
                    self.line("break;");
                } else {
                    self.push_codegen_error(
                        "invalid HIR: `break` outside of loop reached C backend",
                    );
                }
            }
            HirStmt::Continue(_) => {
                if let Some(loop_start) = self.loop_stack.last().copied() {
                    self.emit_cleanups_from(loop_start.0, loop_start.1);
                    self.line("continue;");
                } else {
                    self.push_codegen_error(
                        "invalid HIR: `continue` outside of loop reached C backend",
                    );
                }
            }
            HirStmt::Expr(e) => {
                let s = self.expr_to_c(e);
                self.line(&format!("{};", s));
            }
            HirStmt::If {
                cond,
                then,
                else_ifs,
                else_,
                ..
            } => {
                let cond_s = self.expr_to_c(cond);
                self.line(&format!("if ({}) {{", cond_s));
                self.push();
                self.emit_block(&then.stmts);
                self.pop();
                for (c, b) in else_ifs {
                    let cs = self.expr_to_c(c);
                    self.line(&format!("}} else if ({}) {{", cs));
                    self.push();
                    self.emit_block(&b.stmts);
                    self.pop();
                }
                if let Some(eb) = else_ {
                    self.line("} else {");
                    self.push();
                    self.emit_block(&eb.stmts);
                    self.pop();
                }
                self.line("}");
            }
            HirStmt::While { cond, body, .. } => {
                let cond_s = self.expr_to_c(cond);
                self.line(&format!("while ({}) {{", cond_s));
                self.push();
                self.loop_stack
                    .push((self.using_stack.len(), self.managed_stack.len()));
                self.emit_block(&body.stmts);
                self.loop_stack.pop();
                self.pop();
                self.line("}");
            }
            HirStmt::For {
                binding,
                index_binding,
                elem_ty,
                iterable,
                body,
                ..
            } => {
                match &iterable.kind {
                    HirExprKind::Range { .. } => {
                        // Range for loop
                        let iter_s = self.expr_to_c(iterable);
                        let tmp = self.fresh_tmp();
                        self.line(&format!(
                            "for (int64_t {} = ({}).__start; {} < ({}).__end; {}++) {{",
                            tmp, iter_s, tmp, iter_s, tmp
                        ));
                        self.push();
                        self.line(&format!("int64_t {} = {};", mangle(binding), tmp));
                        if let Some(ib) = index_binding {
                            self.line(&format!(
                                "int64_t {} = {} - ({}).__start;",
                                mangle(ib),
                                tmp,
                                iter_s
                            ));
                        }
                        self.loop_stack
                            .push((self.using_stack.len(), self.managed_stack.len()));
                        self.emit_block(&body.stmts);
                        self.loop_stack.pop();
                        self.pop();
                        self.line("}");
                    }
                    _ if matches!(&iterable.ty, Ty::List(_)) => {
                        // List for loop
                        let list_s = self.expr_to_c(iterable);
                        let list_tmp = self.fresh_tmp();
                        let idx_tmp = self.fresh_tmp();
                        let len_tmp = self.fresh_tmp();
                        let ver_tmp = self.fresh_tmp();
                        let c_elem = ty_to_c(elem_ty);
                        self.line("{");
                        self.push();
                        let (list_ptr_decl, list_ptr_name) = match &iterable.kind {
                            HirExprKind::Var(n) if !self.func_names.contains(n.as_str()) => (
                                format!("ori_list_t* {} = &{};", list_tmp, mangle(n)),
                                list_tmp.clone(),
                            ),
                            _ => {
                                let val_tmp = self.fresh_tmp();
                                (
                                    format!(
                                        "ori_list_t {} = {};\n    ori_list_t* {} = &{};",
                                        val_tmp, list_s, list_tmp, val_tmp
                                    ),
                                    list_tmp.clone(),
                                )
                            }
                        };
                        self.line(&list_ptr_decl);
                        self.line(&format!("size_t {} = {}->version;", ver_tmp, list_ptr_name));
                        self.line(&format!(
                            "int64_t {} = (int64_t){}->len;",
                            len_tmp, list_ptr_name
                        ));
                        self.line(&format!(
                            "for (int64_t {} = 0; {} < {}; {}++) {{",
                            idx_tmp, idx_tmp, len_tmp, idx_tmp
                        ));
                        self.push();
                        self.line(&format!("if ({}->version != {}) {{ ori_abort_bounds(\"concurrent modification during iteration\"); }}", list_ptr_name, ver_tmp));
                        self.line(&format!(
                            "{} {} = *(({}*)ori_list_at({}, (int64_t){}));",
                            c_elem,
                            mangle(binding),
                            c_elem,
                            list_ptr_name,
                            idx_tmp
                        ));
                        if let Some(ib) = index_binding {
                            self.line(&format!("int64_t {} = {};", mangle(ib), idx_tmp));
                        }
                        self.loop_stack
                            .push((self.using_stack.len(), self.managed_stack.len()));
                        self.emit_block(&body.stmts);
                        self.loop_stack.pop();
                        self.pop();
                        self.line("}");
                        self.pop();
                        self.line("}");
                    }
                    _ if matches!(&iterable.ty, Ty::Set(_)) => {
                        // Set for loop
                        let set_s = self.expr_to_c(iterable);
                        let set_tmp = self.fresh_tmp();
                        let idx_tmp = self.fresh_tmp();
                        let len_tmp = self.fresh_tmp();
                        let ver_tmp = self.fresh_tmp();
                        let c_elem = ty_to_c(elem_ty);
                        let value_at = if matches!(elem_ty, Ty::String) {
                            "ori_set_string_at"
                        } else {
                            "ori_set_value_at"
                        };
                        self.line("{");
                        self.push();
                        self.line(&format!("ori_set_t* {} = {};", set_tmp, set_s));
                        self.line(&format!(
                            "size_t {} = {} ? {}->version : 0;",
                            ver_tmp, set_tmp, set_tmp
                        ));
                        self.line(&format!("int64_t {} = ori_set_len({});", len_tmp, set_tmp));
                        self.line(&format!(
                            "for (int64_t {} = 0; {} < {}; {}++) {{",
                            idx_tmp, idx_tmp, len_tmp, idx_tmp
                        ));
                        self.push();
                        self.line(&format!("if ({} && {}->version != {}) {{ ori_abort_bounds(\"concurrent modification during iteration\"); }}", set_tmp, set_tmp, ver_tmp));
                        self.line(&format!(
                            "{} {} = {}({}, {});",
                            c_elem,
                            mangle(binding),
                            value_at,
                            set_tmp,
                            idx_tmp
                        ));
                        if let Some(ib) = index_binding {
                            self.line(&format!("int64_t {} = {};", mangle(ib), idx_tmp));
                        }
                        self.loop_stack
                            .push((self.using_stack.len(), self.managed_stack.len()));
                        self.emit_block(&body.stmts);
                        self.loop_stack.pop();
                        self.pop();
                        self.line("}");
                        self.pop();
                        self.line("}");
                    }
                    _ if matches!(&iterable.ty, Ty::String) => {
                        // String for loop — iterate over chars
                        let str_s = self.expr_to_c(iterable);
                        let chars_tmp = self.fresh_tmp();
                        let idx_tmp = self.fresh_tmp();
                        let len_tmp = self.fresh_tmp();
                        self.line("{");
                        self.push();
                        self.line(&format!(
                            "void* {} = (void*)ori_string_chars({});",
                            chars_tmp, str_s
                        ));
                        self.line(&format!(
                            "int64_t {} = ori_list_len({});",
                            len_tmp, chars_tmp
                        ));
                        self.line(&format!(
                            "for (int64_t {} = 0; {} < {}; {}++) {{",
                            idx_tmp, idx_tmp, len_tmp, idx_tmp
                        ));
                        self.push();
                        self.line(&format!(
                            "const char* {} = (const char*)ori_list_get({}, {});",
                            mangle(binding),
                            chars_tmp,
                            idx_tmp
                        ));
                        if let Some(ib) = index_binding {
                            self.line(&format!("int64_t {} = {};", mangle(ib), idx_tmp));
                        }
                        self.loop_stack
                            .push((self.using_stack.len(), self.managed_stack.len()));
                        self.emit_block(&body.stmts);
                        self.loop_stack.pop();
                        self.pop();
                        self.line("}");
                        self.pop();
                        self.line("}");
                    }
                    _ if matches!(&iterable.ty, Ty::Map(_, _)) => {
                        // Map for loop
                        let Ty::Map(key_ty, value_ty) = &iterable.ty else {
                            unreachable!()
                        };
                        let map_s = self.expr_to_c(iterable);
                        let map_tmp = self.fresh_tmp();
                        let idx_tmp = self.fresh_tmp();
                        let len_tmp = self.fresh_tmp();
                        let ver_tmp = self.fresh_tmp();
                        let c_key = ty_to_c(key_ty);
                        let c_val = ty_to_c(value_ty);
                        let key_at = if matches!(key_ty.as_ref(), Ty::String) {
                            "ori_map_key_string_at"
                        } else {
                            "ori_map_key_at"
                        };
                        self.line("{");
                        self.push();
                        self.line(&format!("ori_map_t* {} = {};", map_tmp, map_s));
                        self.line(&format!(
                            "size_t {} = {} ? {}->version : 0;",
                            ver_tmp, map_tmp, map_tmp
                        ));
                        self.line(&format!("int64_t {} = ori_map_len({});", len_tmp, map_tmp));
                        self.line(&format!(
                            "for (int64_t {} = 0; {} < {}; {}++) {{",
                            idx_tmp, idx_tmp, len_tmp, idx_tmp
                        ));
                        self.push();
                        self.line(&format!("if ({} && {}->version != {}) {{ ori_abort_bounds(\"concurrent modification during iteration\"); }}", map_tmp, map_tmp, ver_tmp));
                        self.line(&format!(
                            "{} {} = {}({}, {});",
                            c_key,
                            mangle(binding),
                            key_at,
                            map_tmp,
                            idx_tmp
                        ));
                        if let Some(ib) = index_binding {
                            self.line(&format!(
                                "{} {} = *(({}*)ori_map_value_at({}, {}));",
                                c_val,
                                mangle(ib),
                                c_val,
                                map_tmp,
                                idx_tmp
                            ));
                        }
                        self.loop_stack
                            .push((self.using_stack.len(), self.managed_stack.len()));
                        self.emit_block(&body.stmts);
                        self.loop_stack.pop();
                        self.pop();
                        self.line("}");
                        self.pop();
                        self.line("}");
                    }
                    _ if matches!(&iterable.ty, Ty::Opaque { kind, .. } if kind.is_list_backed_collection() || matches!(kind, OpaqueTy::Heap | OpaqueTy::Graph)) =>
                    {
                        let Ty::Opaque { kind, .. } = &iterable.ty else {
                            unreachable!()
                        };
                        let prefix = match kind {
                            OpaqueTy::Deque => "ori_deque_iterator",
                            OpaqueTy::Queue => "ori_queue_iterator",
                            OpaqueTy::Stack => "ori_stack_iterator",
                            OpaqueTy::LinkedList => "ori_linked_list_iterator",
                            OpaqueTy::DoublyLinkedList => "ori_doubly_linked_list_iterator",
                            OpaqueTy::Heap => "ori_heap_iterator",
                            OpaqueTy::Graph => "ori_graph_iterator",
                            _ => unreachable!(),
                        };
                        let col_s = self.expr_to_c(iterable);
                        let iter_tmp = self.fresh_tmp();
                        let idx_tmp = self.fresh_tmp();
                        let c_elem = ty_to_c(elem_ty);
                        let elem_ptr_tmp = self.fresh_tmp();
                        self.line("{");
                        self.push();
                        self.line(&format!(
                            "void* {} = {}_{}({});",
                            iter_tmp, prefix, "new", col_s
                        ));
                        let iter_ty = Ty::Opaque {
                            kind: *kind,
                            args: vec![],
                        };
                        self.managed_stack.push((iter_tmp.clone(), iter_ty.clone()));
                        if index_binding.is_some() {
                            self.line(&format!("int64_t {} = 0;", idx_tmp));
                        }
                        self.line("for (;;) {");
                        self.push();
                        self.line(&format!(
                            "{}* {} = {}_{}({});",
                            c_elem, elem_ptr_tmp, prefix, "next", iter_tmp
                        ));
                        self.line(&format!("if (!{}) break;", elem_ptr_tmp));
                        self.line(&format!(
                            "{} {} = *{};",
                            c_elem,
                            mangle(binding),
                            elem_ptr_tmp
                        ));
                        if let Some(ib) = index_binding {
                            self.line(&format!("int64_t {} = {};", mangle(ib), idx_tmp));
                        }
                        self.loop_stack
                            .push((self.using_stack.len(), self.managed_stack.len()));
                        self.emit_block(&body.stmts);
                        self.loop_stack.pop();
                        if index_binding.is_some() {
                            self.line(&format!("{}++;", idx_tmp));
                        }
                        self.pop();
                        self.line("}");
                        let managed_cleanup_start = self.managed_stack.len() - 1;
                        self.emit_cleanups_from(self.using_stack.len(), managed_cleanup_start);
                        self.managed_stack.truncate(managed_cleanup_start);
                        self.pop();
                        self.line("}");
                    }
                    _ => {
                        self.push_codegen_error(format!(
                            "C backend does not support for-loop iterable type `{}`",
                            iterable.ty.display()
                        ));
                    }
                }
            }
            HirStmt::Loop { body, .. } => {
                self.line("for (;;) {");
                self.push();
                self.loop_stack
                    .push((self.using_stack.len(), self.managed_stack.len()));
                self.emit_block(&body.stmts);
                self.loop_stack.pop();
                self.pop();
                self.line("}");
            }
            HirStmt::Repeat { count, body, .. } => {
                let count_s = self.expr_to_c(count);
                let count_tmp = self.fresh_tmp();
                let tmp = self.fresh_tmp();
                self.line(&format!("int64_t {} = {};", count_tmp, count_s));
                self.line(&format!(
                    "if ({} < 0) {{ fprintf(stderr, \"ori repeat count is negative\\n\"); abort(); }}",
                    count_tmp
                ));
                self.line(&format!(
                    "for (int64_t {} = 0; {} < ({}); {}++) {{",
                    tmp, tmp, count_tmp, tmp
                ));
                self.push();
                self.loop_stack
                    .push((self.using_stack.len(), self.managed_stack.len()));
                self.emit_block(&body.stmts);
                self.loop_stack.pop();
                self.pop();
                self.line("}");
            }
            HirStmt::Match {
                scrutinee, arms, ..
            } => {
                let scr = self.expr_to_c(scrutinee);
                let tmp = self.fresh_tmp();
                self.line(&format!(
                    "{{ {} {} = {}; (void){};",
                    ty_to_c(&scrutinee.ty),
                    tmp,
                    scr,
                    tmp
                ));
                if arms.iter().any(|arm| arm.guard.is_some()) {
                    // Guarded arms need "pattern matched but guard failed →
                    // try the next arm", which an if/else chain cannot
                    // express: bindings are declared inside the arm block
                    // and the guard reads them. Emit each arm as a plain
                    // `if` whose body jumps to the end label; a false guard
                    // jumps past this arm's block to the next test. Labels
                    // (not do/while+break) so `break` in an arm body still
                    // binds to the enclosing Ori loop.
                    for (i, arm) in arms.iter().enumerate() {
                        let cond = pattern_cond(&arm.pattern, &tmp);
                        self.line(&format!("if ({}) {{", cond));
                        self.push();
                        emit_pattern_bindings(&arm.pattern, &tmp, &mut self.out, self.indent);
                        if let Some(guard) = &arm.guard {
                            let guard_c = self.expr_to_c(guard);
                            self.line(&format!("if (!({})) goto {}_next{};", guard_c, tmp, i));
                        }
                        self.emit_block(&arm.body);
                        self.line(&format!("goto {}_end;", tmp));
                        self.pop();
                        self.line("}");
                        if arm.guard.is_some() {
                            self.line(&format!("{}_next{}:;", tmp, i));
                        }
                    }
                    self.line(&format!("{}_end:;", tmp));
                    self.line("}");
                } else {
                    // Emit as if-else chain
                    for (i, arm) in arms.iter().enumerate() {
                        let cond = pattern_cond(&arm.pattern, &tmp);
                        if i == 0 {
                            self.line(&format!("if ({}) {{", cond));
                        } else if cond == "1" {
                            self.line("} else {");
                        } else {
                            self.line(&format!("}} else if ({}) {{", cond));
                        }
                        self.push();
                        // Bind pattern variables
                        emit_pattern_bindings(&arm.pattern, &tmp, &mut self.out, self.indent);
                        self.emit_block(&arm.body);
                        self.pop();
                    }
                    self.line("} }");
                }
            }
            HirStmt::IfSome {
                binding,
                inner_ty,
                value,
                then,
                else_,
                ..
            } => {
                // Desugar:
                //   { auto _tmp = <value>; if (_tmp.has_value) { T binding = _tmp.value; ... } else { ... } }
                let val_s = self.expr_to_c(value);
                let tmp = self.fresh_tmp();
                let opt_ty = ty_to_c(&Ty::Optional(Box::new(inner_ty.clone())));
                let val_ty = ty_to_c(inner_ty);
                self.line("{");
                self.push();
                self.line(&format!("{} {} = {};", opt_ty, tmp, val_s));
                self.line(&format!("if ({}.has_value) {{", tmp));
                self.push();
                self.line(&format!("{} {} = {}.value;", val_ty, mangle(binding), tmp));
                self.emit_block(&then.stmts);
                self.pop();
                if let Some(eb) = else_ {
                    self.line("} else {");
                    self.push();
                    self.emit_block(&eb.stmts);
                    self.pop();
                }
                self.line("}");
                self.pop();
                self.line("}");
            }
            HirStmt::WhileSome {
                binding,
                inner_ty,
                value,
                body,
                ..
            } => {
                // Desugar:
                //   for (;;) { auto _tmp = <value>; if (!_tmp.has_value) break; T binding = _tmp.value; ... }
                let tmp = self.fresh_tmp();
                let opt_ty = ty_to_c(&Ty::Optional(Box::new(inner_ty.clone())));
                let val_ty = ty_to_c(inner_ty);
                self.line("for (;;) {");
                self.push();
                let val_s = self.expr_to_c(value);
                self.line(&format!("{} {} = {};", opt_ty, tmp, val_s));
                self.line(&format!("if (!{}.has_value) break;", tmp));
                self.line(&format!("{} {} = {}.value;", val_ty, mangle(binding), tmp));
                self.loop_stack
                    .push((self.using_stack.len(), self.managed_stack.len()));
                self.emit_block(&body.stmts);
                self.loop_stack.pop();
                self.pop();
                self.line("}");
            }
            HirStmt::Using {
                name, ty, value, ..
            } => {
                let val_s = self.expr_to_c(value);
                self.line(&format!("{} {} = {};", ty_to_c(ty), mangle(name), val_s));
                self.using_stack
                    .push((name.clone().to_string(), ty.clone()));
            }
            HirStmt::Check {
                condition, message, ..
            } => {
                let cond_s = self.expr_to_c(condition);
                let msg = message.as_deref().unwrap_or("check failed");
                self.line(&format!("if (!({cond_s})) {{ fprintf(stderr, \"ori check failed: {msg}\\n\"); abort(); }}"));
            }
        }
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    fn expr_to_c_for_expected(&mut self, expr: &HirExpr, expected: &Ty) -> String {
        let val_s = self.expr_to_c(expr);
        if let (Ty::Optional(expected_inner), Ty::Optional(_)) = (expected, &expr.ty) {
            if matches!(expr.kind, HirExprKind::None_) {
                return format!("(({}){{ .has_value = false }})", ty_to_c(expected));
            } else if let HirExprKind::Some_(inner) = &expr.kind {
                let i = self.expr_to_c_for_expected(inner, expected_inner);
                return format!(
                    "(({}){{ .has_value = true, .value = {} }})",
                    ty_to_c(expected),
                    i
                );
            }
        }
        if let (Ty::Any(trait_def_id), Ty::Named(type_def_id, _)) = (expected, &expr.ty) {
            let Some(trait_layout) = self.trait_layouts.get(trait_def_id).cloned() else {
                return self.unsupported_expr(format!(
                    "C backend cannot box `{}` as any: missing trait layout for def {}",
                    expr.ty.display(),
                    trait_def_id.0
                ));
            };
            let Some(impl_sig) = self
                .trait_impls
                .get(&(*trait_def_id, *type_def_id))
                .cloned()
            else {
                return self.unsupported_expr(format!(
                    "C backend cannot box `{}` as any: missing implementation for trait def {}",
                    expr.ty.display(),
                    trait_def_id.0
                ));
            };

            let mut vtable_entries = vec![format!("(void*){}", type_def_id.0)];
            if self.type_supports_equality(&Ty::Named(*type_def_id, Vec::new()), &mut Vec::new()) {
                vtable_entries.push(format!("(void*)__eq_helper_struct_{}", type_def_id.0));
            } else {
                vtable_entries.push("NULL".to_string());
            }
            for method in &trait_layout.methods {
                let Some(func_name) = impl_sig
                    .methods
                    .iter()
                    .find(|m| m.name == method.name)
                    .map(|m| m.func_name.clone())
                    .or_else(|| method.default_func_name.clone())
                else {
                    return self.unsupported_expr(format!(
                        "C backend cannot box `{}` as any: missing method `{}` for trait def {}",
                        expr.ty.display(),
                        method.name,
                        trait_def_id.0
                    ));
                };
                vtable_entries.push(format!("(void*){}", Self::func_c_name(&func_name)));
            }

            let vtable_tmp = self.fresh_tmp();
            let any_tmp = self.fresh_tmp();
            let obj_tmp = self.fresh_tmp();
            let type_name = def_c_name(*type_def_id);
            let mut parts = Vec::new();

            parts.push(format!(
                "void* {}[] = {{ {} }}",
                vtable_tmp,
                vtable_entries.join(", ")
            ));
            // Box the value on the heap using ori_alloc (since any<Trait> is a managed type, its contents might need disposing but the actual ori_any_t holds the ptr)
            // But wait, any<Trait> in C needs a heap allocation for the `obj`.
            parts.push(format!(
                "{}* {} = ({}*)ori_alloc(sizeof({}), 0)",
                type_name, obj_tmp, type_name, type_name
            ));
            parts.push(format!("if ({}) *{} = {}", obj_tmp, obj_tmp, val_s));
            parts.push(format!(
                "ori_any_t {} = {{ .obj = (void*){}, .vtable = {} }}",
                any_tmp, obj_tmp, vtable_tmp
            ));

            format!(
                "({{ {}; {}; {}; {}; {}; }})",
                parts[0], parts[1], parts[2], parts[3], any_tmp
            )
        } else if let (Ty::Named(expected_id, _), Ty::Named(actual_id, _)) = (expected, &expr.ty) {
            if expected_id != actual_id && self.trait_layouts.contains_key(expected_id) {
                // We are passing a concrete struct to a default trait method expecting the trait type by value.
                // Since the trait type has no fields in C, we can just pass an empty struct.
                format!("(({}){{0}})", def_c_name(*expected_id))
            } else {
                val_s
            }
        } else {
            val_s
        }
    }

    fn expr_to_c(&mut self, expr: &HirExpr) -> String {
        match &expr.kind {
            HirExprKind::BoolLit(b) => {
                if *b {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            HirExprKind::IntLit(n) => format!("INT64_C({})", n),
            HirExprKind::FloatLit(f) => float_lit_to_c(*f),
            HirExprKind::StrLit(s) => format!("ORI_STR(\"{}\")", escape_c_str(s)),
            HirExprKind::Unit => "((void)0)".into(),
            HirExprKind::None_ => format!("(({}){{ .has_value = false }})", ty_to_c(&expr.ty)),
            HirExprKind::Var(n) => {
                // Top-level functions get the ORI__ prefix; local vars don't
                if self.func_names.contains(n.as_str()) {
                    Self::func_c_name(n)
                } else {
                    mangle(n)
                }
            }
            HirExprKind::Binary { op, lhs, rhs } => {
                let l = self.expr_to_c(lhs);
                let r = self.expr_to_c(rhs);
                if matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
                    return self.equality_to_c(l, r, &lhs.ty, matches!(op, BinaryOp::Eq));
                }
                let is_str = matches!(&lhs.ty, Ty::String) || matches!(&rhs.ty, Ty::String);
                if is_str {
                    match op {
                        BinaryOp::Add => format!("ori_string_concat({}, {})", l, r),
                        _ => format!("({} {} {})", l, binop_to_c(*op), r),
                    }
                } else {
                    format!("({} {} {})", l, binop_to_c(*op), r)
                }
            }
            HirExprKind::Unary { op, operand } => {
                let e = self.expr_to_c(operand);
                match op {
                    UnaryOp::Neg => format!("(-{})", e),
                    UnaryOp::Not => format!("(!{})", e),
                }
            }
            HirExprKind::Field { object, field } => {
                let obj = self.expr_to_c(object);
                format!("{}.{}", obj, mangle(field))
            }
            HirExprKind::TupleIndex { object, index } => {
                let obj = self.expr_to_c(object);
                format!("{}._f{}", obj, index)
            }
            HirExprKind::Call { callee, args } => {
                if let HirExprKind::Var(n) = &callee.kind {
                    if is_concurrency_runtime_symbol(n.as_str()) {
                        return self.unsupported_expr(
                            "C backend does not support concurrency/async runtime calls yet; use the native backend",
                        );
                    }
                    if n.as_str() == "ori_lazy_once" && args.len() == 1 {
                        return self.emit_lazy_once(&args[0].value, &expr.ty);
                    }
                    if n.as_str() == "ori_lazy_force" && args.len() == 1 {
                        return self.emit_lazy_force(&args[0].value, &expr.ty);
                    }
                    if n.as_str() == "__ori_builtin_or" && args.len() == 2 {
                        return self.emit_builtin_or(&args[0].value, &args[1].value);
                    }
                    if n.as_str() == "__ori_builtin_or_wrap" && args.len() == 2 {
                        return self.emit_builtin_or_wrap(&args[0].value, &args[1].value);
                    }
                    if is_map_runtime_symbol(n.as_str()) {
                        return self.emit_map_runtime_call(n.as_str(), args, &expr.ty);
                    }
                    if is_set_runtime_symbol(n.as_str()) {
                        return self.emit_set_runtime_call(n.as_str(), args, &expr.ty);
                    }
                }
                let params = match &callee.ty {
                    Ty::Func { params, .. } => params.clone(),
                    _ => vec![],
                };
                let callee_s = self.expr_to_c(callee);

                let mut args_s = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let expected = params.get(i).unwrap_or(&arg.value.ty);
                    args_s.push(self.expr_to_c_for_expected(&arg.value, expected));
                }

                // Determine if this is a direct named-function call or a closure variable call.
                // Direct calls: Ori functions in func_names, runtime stdlib (ori_ prefix), externs.
                // Closure calls: local variables of type Func that hold a closure struct.
                let is_direct = match &callee.kind {
                    HirExprKind::Var(n) => {
                        self.func_names.contains(n.as_str()) // Ori / extern function
                        || n.starts_with("ori_") // stdlib runtime (ori_io_print, etc.)
                    }
                    _ => false,
                };

                // Special-case: iter/list helpers expand closure arg to (fn_ptr, env_ptr).
                if let HirExprKind::Var(n) = &callee.kind {
                    if n.as_str() == "ori_list_push" && args.len() == 2 {
                        let list_c = self.expr_to_c(&args[0].value);
                        let item_c = self.expr_to_c(&args[1].value);
                        let item_ty_c = ty_to_c(&args[1].value.ty);
                        let tmp_list = self.fresh_tmp();
                        let tmp_item = self.fresh_tmp();
                        return format!("({{ ori_list_t {tmp_list} = {list_c}; {item_ty_c} {tmp_item} = {item_c}; ori_list_push(&{tmp_list}, &{tmp_item}); }})");
                    }
                    if matches!(
                        n.as_str(),
                        "ori_list_try_get"
                            | "ori_list_get"
                            | "ori_list_remove"
                            | "ori_list_pop"
                            | "ori_list_insert"
                            | "ori_bytes_from_hex"
                            | "ori_io_read_line"
                            | "ori_io_read_bytes"
                            | "ori_io_try_read_line"
                            | "ori_io_try_read_bytes"
                            | "ori_io_read_all"
                            | "ori_io_try_read_all"
                            | "ori_io_read"
                            | "ori_string_from_bytes"
                            | "ori_string_to_bytes"
                            | "ori_random_choice"
                            | "ori_random_int"
                            | "ori_random_float"
                            | "ori_os_args"
                            | "ori_os_current_dir"
                            | "ori_os_read_env"
                            | "ori_io_write"
                            | "ori_io_flush"
                            | "ori_bytes_concat"
                            | "ori_bytes_to_list"
                    ) {
                        let ret_ty_c = ty_to_c(&expr.ty);
                        if ret_ty_c == "void" {
                            return format!("({{}})");
                        } else {
                            return format!("(({ret_ty_c}){{0}})");
                        }
                    }
                    if n.as_str() == "ori_string_parse_int" && args.len() == 1 {
                        return self.emit_string_parse_int(&args_s[0], &expr.ty);
                    }
                    if n.as_str() == "ori_string_parse_float" && args.len() == 1 {
                        return self.emit_string_parse_float(&args_s[0], &expr.ty);
                    }
                    if matches!(n.as_str(), "ori_test_assert_eq" | "ori_test_assert_ne")
                        && args.len() == 2
                    {
                        let is_ne = n.as_str() == "ori_test_assert_ne";
                        let runtime_name = match &args[0].value.ty {
                            Ty::String => {
                                if is_ne {
                                    "ori_test_assert_ne_string"
                                } else {
                                    "ori_test_assert_eq_string"
                                }
                            }
                            Ty::Float | Ty::Float32 | Ty::Float64 => {
                                if is_ne {
                                    "ori_test_assert_ne_float"
                                } else {
                                    "ori_test_assert_eq_float"
                                }
                            }
                            Ty::Bool => {
                                if is_ne {
                                    "ori_test_assert_ne_bool"
                                } else {
                                    "ori_test_assert_eq_bool"
                                }
                            }
                            _ => n.as_str(),
                        };
                        let left = self.expr_to_c_for_expected(&args[0].value, &args[0].value.ty);
                        let right = self.expr_to_c_for_expected(&args[1].value, &args[0].value.ty);
                        return format!("{runtime_name}({left}, {right})");
                    }
                    if matches!(n.as_str(), "ori_iter_sort" | "ori_iter_unique")
                        && args.len() == 1
                        && matches!(
                            &args[0].value.ty,
                            Ty::List(elem) if matches!(elem.as_ref(), Ty::String)
                        )
                    {
                        let runtime_name = if n.as_str() == "ori_iter_sort" {
                            "ori_iter_sort_string"
                        } else {
                            "ori_iter_unique_string"
                        };
                        let list_s = self.expr_to_c(&args[0].value);
                        return format!("{runtime_name}({list_s})");
                    }
                    if matches!(
                        n.as_str(),
                        "ori_list_map"
                            | "ori_list_filter"
                            | "ori_iter_flat_map"
                            | "ori_iter_any"
                            | "ori_iter_all"
                            | "ori_iter_count_where"
                            | "ori_iter_find"
                            | "ori_iter_partition"
                            | "ori_iter_group_by"
                    ) && args.len() == 2
                        && matches!(&args[1].value.ty, Ty::Func { .. })
                    {
                        let list_s = self.expr_to_c(&args[0].value);
                        let fn_expr = self.expr_to_c(&args[1].value);
                        let runtime_name = if n.as_str() == "ori_iter_group_by"
                            && matches!(
                                &expr.ty,
                                Ty::Map(key, _) if matches!(key.as_ref(), Ty::String)
                            ) {
                            "ori_iter_group_by_string"
                        } else {
                            n.as_str()
                        };
                        return format!(
                            "{}({}, {}->fn_ptr, {}->env_ptr)",
                            runtime_name, list_s, fn_expr, fn_expr
                        );
                    }
                    if n.as_str() == "ori_iter_reduce"
                        && args.len() == 3
                        && matches!(&args[2].value.ty, Ty::Func { .. })
                    {
                        let list_s = self.expr_to_c(&args[0].value);
                        let initial_s = self.expr_to_c(&args[1].value);
                        let fn_expr = self.expr_to_c(&args[2].value);
                        return format!(
                            "{}({}, {}, {}->fn_ptr, {}->env_ptr)",
                            n, list_s, initial_s, fn_expr, fn_expr
                        );
                    }
                    if n.as_str() == "ori_iter_sort_by"
                        && args.len() == 2
                        && matches!(&args[1].value.ty, Ty::Func { .. })
                    {
                        let list_s = self.expr_to_c(&args[0].value);
                        let fn_expr = self.expr_to_c(&args[1].value);
                        return format!(
                            "{}({}, {}->fn_ptr, {}->env_ptr)",
                            n, list_s, fn_expr, fn_expr
                        );
                    }
                }

                if !is_direct && matches!(&callee.ty, Ty::Func { .. }) {
                    // Closure call: callee is a local `ori_closure_t*` variable.
                    let mut all_args = vec![format!("{}->env_ptr", callee_s)];
                    all_args.extend(args_s);
                    let ret_ty = if let Ty::Func { ret, .. } = &callee.ty {
                        ty_to_c(ret)
                    } else {
                        "void".to_string()
                    };
                    let params_ty: Vec<String> = if let Ty::Func { params, .. } = &callee.ty {
                        let mut p = vec!["void*".to_string()];
                        p.extend(params.iter().map(|t| ty_to_c(t)));
                        p
                    } else {
                        vec!["void*".to_string()]
                    };
                    let fn_cast = format!(
                        "(({} (*)({})){}->fn_ptr)",
                        ret_ty,
                        params_ty.join(", "),
                        callee_s
                    );
                    format!("{}({})", fn_cast, all_args.join(", "))
                } else {
                    // Direct call to a named function.
                    format!("{}({})", callee_s, args_s.join(", "))
                }
            }
            HirExprKind::IfExpr { cond, then, else_ } => {
                let c = self.expr_to_c(cond);
                let t = self.expr_to_c(then);
                let e = self.expr_to_c(else_);
                format!("({} ? {} : {})", c, t, e)
            }
            HirExprKind::Some_(inner) => {
                let expected_inner = match &expr.ty {
                    Ty::Optional(inner_ty) => &**inner_ty,
                    _ => &inner.ty,
                };
                let i = self.expr_to_c_for_expected(inner, expected_inner);
                format!(
                    "(({}){{ .has_value = true, .value = {} }})",
                    ty_to_c(&expr.ty),
                    i
                )
            }
            HirExprKind::Propagate(inner) => {
                let inner_s = self.expr_to_c(inner);
                let tmp = self.fresh_tmp();
                let cleanup = self.cleanup_statements_from(0, 0).join(" ");
                let Some(return_ty) = self.current_return_ty.clone() else {
                    return self.unsupported_expr(
                        "C backend cannot lower `?` outside a function return context",
                    );
                };
                match (&inner.ty, &return_ty) {
                    (Ty::Result(_, inner_err), Ty::Result(_, return_err))
                        if inner_err.is_assignable_to(return_err) =>
                    {
                        format!(
                            "({{ {} {} = {}; if (!{}.is_ok) {{ {} return (({}){{ .is_ok = false, .value.err = {}.value.err }}); }} {}.value.ok; }})",
                            ty_to_c(&inner.ty),
                            tmp,
                            inner_s,
                            tmp,
                            cleanup,
                            ty_to_c(&return_ty),
                            tmp,
                            tmp
                        )
                    }
                    (Ty::Optional(_), Ty::Optional(_)) => {
                        format!(
                            "({{ {} {} = {}; if (!{}.has_value) {{ {} return (({}){{ .has_value = false }}); }} {}.value; }})",
                            ty_to_c(&inner.ty),
                            tmp,
                            inner_s,
                            tmp,
                            cleanup,
                            ty_to_c(&return_ty),
                            tmp
                        )
                    }
                    _ => self.unsupported_expr(format!(
                        "C backend cannot propagate `{}` from function returning `{}`",
                        inner.ty.display(),
                        return_ty.display()
                    )),
                }
            }
            HirExprKind::InterpolatedStr(parts) => self.emit_interp_str(parts),
            HirExprKind::BytesLit(bytes) => {
                let elems: Vec<String> = bytes.iter().map(|b| format!("0x{:02x}", b)).collect();
                format!("((uint8_t[]){{ {} }})", elems.join(", "))
            }
            HirExprKind::ListLit { elem_ty, elements } => {
                let c_elem_ty = ty_to_c(elem_ty);
                if elements.is_empty() {
                    format!("ori_list_new(sizeof({}))", c_elem_ty)
                } else {
                    // Build inline: create list, push elements via statement expression
                    let tmp = self.fresh_tmp();
                    let mut parts = Vec::new();
                    parts.push(format!(
                        "ori_list_t {} = ori_list_new(sizeof({}))",
                        tmp, c_elem_ty
                    ));
                    for elem in elements {
                        let val = self.expr_to_c(elem);
                        let elem_tmp = self.fresh_tmp();
                        parts.push(format!("{} {} = {}", c_elem_ty, elem_tmp, val));
                        parts.push(format!("ori_list_push(&{}, &{})", tmp, elem_tmp));
                    }
                    format!("({{ {}; {}; }})", parts.join("; "), tmp)
                }
            }
            HirExprKind::ListSpreadLit { elem_ty, elements } => {
                let c_elem_ty = ty_to_c(elem_ty);
                let tmp = self.fresh_tmp();
                let mut parts = Vec::new();
                parts.push(format!(
                    "ori_list_t {} = ori_list_new(sizeof({}))",
                    tmp, c_elem_ty
                ));
                for elem in elements {
                    let val = self.expr_to_c(&elem.value);
                    if elem.spread {
                        let src_tmp = self.fresh_tmp();
                        let idx_tmp = self.fresh_tmp();
                        let elem_tmp = self.fresh_tmp();
                        parts.push(format!("ori_list_t {} = {}", src_tmp, val));
                        parts.push(format!(
                            "for (size_t {} = 0; {} < {}.len; {}++) {{ {} {} = *(({}*)ori_list_at(&{}, (int64_t){})); ori_list_push(&{}, &{}); }}",
                            idx_tmp,
                            idx_tmp,
                            src_tmp,
                            idx_tmp,
                            c_elem_ty,
                            elem_tmp,
                            c_elem_ty,
                            src_tmp,
                            idx_tmp,
                            tmp,
                            elem_tmp,
                        ));
                    } else {
                        let elem_tmp = self.fresh_tmp();
                        parts.push(format!("{} {} = {}", c_elem_ty, elem_tmp, val));
                        parts.push(format!("ori_list_push(&{}, &{})", tmp, elem_tmp));
                    }
                }
                format!("({{ {}; {}; }})", parts.join("; "), tmp)
            }
            HirExprKind::TupleLit(elems) => {
                let mut field_inits = Vec::new();
                for (i, elem) in elems.iter().enumerate() {
                    let val = self.expr_to_c(elem);
                    field_inits.push(format!("._f{} = {}", i, val));
                }
                format!("(({}){{ {} }})", ty_to_c(&expr.ty), field_inits.join(", "))
            }
            HirExprKind::Range { start, end } => {
                let s = self.expr_to_c(start);
                let e = self.expr_to_c(end);
                format!("((ori_range_t){{ .__start = {}, .__end = {} }})", s, e)
            }
            HirExprKind::StructLit { def_id, fields } => {
                let fields_s: Vec<String> = fields
                    .iter()
                    .map(|(n, e)| {
                        let es = self.expr_to_c(e);
                        format!(".{} = {}", mangle(n), es)
                    })
                    .collect();
                if def_id.0 != u32::MAX {
                    format!("(({}){{ {} }})", def_c_name(*def_id), fields_s.join(", "))
                } else {
                    format!("({{ {} }})", fields_s.join(", "))
                }
            }
            HirExprKind::EnumVariant {
                def_id,
                variant,
                fields,
            } => {
                let type_name = def_c_name(*def_id);
                let tag = format!("{}__{}", type_name, mangle(variant));
                if fields.is_empty() {
                    format!("(({}){{ .tag = {} }})", type_name, tag)
                } else {
                    let fields_s: Vec<String> = fields
                        .iter()
                        .map(|(n, e)| {
                            let es = self.expr_to_c(e);
                            format!(".{} = {}", mangle(n), es)
                        })
                        .collect();
                    format!(
                        "(({}){{ .tag = {}, .payload.{} = {{ {} }} }})",
                        type_name,
                        tag,
                        mangle(variant),
                        fields_s.join(", ")
                    )
                }
            }
            HirExprKind::Await(_) => self
                .unsupported_expr("C backend does not support `await` yet; use the native backend"),
            HirExprKind::Ok_(inner) => {
                let expected_inner = match &expr.ty {
                    Ty::Result(ok, _) => &**ok,
                    _ => &inner.ty,
                };
                let i = self.expr_to_c_for_expected(inner, expected_inner);
                format!(
                    "(({}){{ .is_ok = true, .value.ok = {} }})",
                    ty_to_c(&expr.ty),
                    i
                )
            }
            HirExprKind::Err_(inner) => {
                let expected_inner = match &expr.ty {
                    Ty::Result(_, err) => &**err,
                    _ => &inner.ty,
                };
                let i = self.expr_to_c_for_expected(inner, expected_inner);
                format!(
                    "(({}){{ .is_ok = false, .value.err = {} }})",
                    ty_to_c(&expr.ty),
                    i
                )
            }
            HirExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                let r = self.expr_to_c(receiver);
                let as_: Vec<String> = args.iter().map(|a| self.expr_to_c(a)).collect();
                if method == "__slice" {
                    if matches!(&receiver.ty, Ty::String) {
                        return format!("ori_string_slice({}, {})", r, as_.join(", "));
                    } else if matches!(&receiver.ty, Ty::List(_)) {
                        return format!("ori_list_slice({}, {})", r, as_.join(", "));
                    }
                }

                if let Ty::Any(trait_def_id) = &receiver.ty {
                    let Some(trait_layout) = self.trait_layouts.get(trait_def_id).cloned() else {
                        return self.unsupported_expr(format!(
                            "C backend cannot dispatch `{}`: missing trait layout for def {}",
                            method, trait_def_id.0
                        ));
                    };
                    let Some(method_index) =
                        trait_layout.methods.iter().position(|m| m.name == *method)
                    else {
                        return self.unsupported_expr(format!(
                            "C backend cannot dispatch `{}`: method is absent from trait def {}",
                            method, trait_def_id.0
                        ));
                    };
                    let method_sig = &trait_layout.methods[method_index];
                    let ret_ty = ty_to_c(&method_sig.return_ty);
                    let mut params_ty = vec!["void*".to_string()];
                    params_ty.extend(method_sig.params.iter().skip(1).map(|t| ty_to_c(t)));

                    let mut call_args = vec![format!("({}).obj", r)];
                    call_args.extend(as_);

                    let fn_cast = format!(
                        "(({} (*)({}))(((void**)({}).vtable)[{}]))",
                        ret_ty,
                        params_ty.join(", "),
                        r,
                        method_index + 2
                    );

                    format!(
                        "({{ ori_arc_retain(({}).obj); {}({}); }})",
                        r,
                        fn_cast,
                        call_args.join(", ")
                    )
                } else {
                    format!("ori__{}({}, {})", mangle(method), r, as_.join(", "))
                }
            }
            HirExprKind::Index { object, index } => {
                let o = self.expr_to_c(object);
                let i = self.expr_to_c(index);
                match &object.ty {
                    Ty::List(_) => format!(
                        "*(({}*)ori_list_at(&{}, (int64_t){}))",
                        ty_to_c(&expr.ty),
                        o,
                        i
                    ),
                    Ty::String => format!("ori_string_get({}, (int64_t){})", o, i),
                    Ty::Bytes => self.unsupported_expr(
                        "C backend does not support byte indexing yet; use the native backend"
                            .to_string(),
                    ),
                    other => self.unsupported_expr(format!(
                        "C backend does not support `{}` as index expression",
                        other.display()
                    )),
                }
            }
            HirExprKind::MapLit {
                key_ty,
                value_ty,
                entries,
            } => {
                if !matches!(key_ty, Ty::Int | Ty::String) {
                    return self.unsupported_expr(format!(
                        "C backend cannot lower map literal with `{}` keys yet",
                        key_ty.display()
                    ));
                }
                let tmp = self.fresh_tmp();
                let mut parts = Vec::new();
                parts.push(format!("ori_map_t* {} = ori_map_new()", tmp));
                for (k, v) in entries {
                    let ks = self.map_key_to_c(k, key_ty);
                    let vs = self.expr_to_c_for_expected(v, value_ty);
                    let value_tmp = self.fresh_tmp();
                    let value_c = ty_to_c(value_ty);
                    let runtime_name = if matches!(key_ty, Ty::String) {
                        "ori_map_set_string_value"
                    } else {
                        "ori_map_set_value"
                    };
                    parts.push(format!("{value_c} {value_tmp} = {vs}"));
                    parts.push(format!(
                        "{runtime_name}({}, {}, &{}, sizeof({}))",
                        tmp, ks, value_tmp, value_tmp
                    ));
                }
                format!("({{ {}; {}; }})", parts.join("; "), tmp)
            }
            HirExprKind::SetLit { elem_ty, elements } => {
                if !matches!(elem_ty, Ty::Int | Ty::String) {
                    return self.unsupported_expr(format!(
                        "C backend cannot lower set literal with `{}` values yet",
                        elem_ty.display()
                    ));
                }
                let tmp = self.fresh_tmp();
                let mut parts = Vec::new();
                parts.push(format!("ori_set_t* {} = ori_set_new()", tmp));
                let runtime_name = if matches!(elem_ty, Ty::String) {
                    "ori_set_add_string"
                } else {
                    "ori_set_add"
                };
                for elem in elements {
                    let es = self.set_key_to_c(elem, elem_ty);
                    parts.push(format!("{runtime_name}({}, {})", tmp, es));
                }
                format!("({{ {}; {}; }})", parts.join("; "), tmp)
            }
            HirExprKind::StructUpdate {
                def_id,
                base,
                updates,
            } => {
                let base_s = self.expr_to_c(base);
                let type_name = def_c_name(*def_id);
                let tmp = self.fresh_tmp();
                let overrides: Vec<String> = updates
                    .iter()
                    .map(|(n, e)| {
                        let es = self.expr_to_c(e);
                        format!("{}.{} = {}", tmp, mangle(n), es)
                    })
                    .collect();
                format!(
                    "({{ {} {} = {}; {}; {}; }})",
                    type_name,
                    tmp,
                    base_s,
                    overrides.join("; "),
                    tmp
                )
            }
            HirExprKind::IsCheck { value, check_ty } => {
                let val_s = self.expr_to_c(value);
                let tmp = self.fresh_tmp();
                let result = if let Ty::Named(check_def_id, _) = check_ty {
                    match &value.ty {
                        Ty::Any(_) => format!(
                            "(((void**){}.vtable)[0] == (void*)(intptr_t){})",
                            tmp, check_def_id.0
                        ),
                        Ty::Named(actual_def_id, _) => c_bool(actual_def_id == check_def_id),
                        _ => c_bool(false),
                    }
                } else {
                    c_bool(value.ty == *check_ty)
                };
                format!(
                    "({{ {} {} = {}; {}; }})",
                    ty_to_c(&value.ty),
                    tmp,
                    val_s,
                    result
                )
            }
            HirExprKind::Closure {
                func_name,
                captures,
            } => {
                let tmp = self.fresh_tmp();
                let mut parts = Vec::new();
                parts.push(format!(
                    "ori_closure_t* {} = (ori_closure_t*)ori_alloc(sizeof(ori_closure_t), 0)",
                    tmp
                ));
                parts.push(format!(
                    "{}->fn_ptr = (void*){}",
                    tmp,
                    Self::func_c_name(func_name)
                ));
                if captures.is_empty() {
                    parts.push(format!("{}->env_ptr = NULL", tmp));
                } else {
                    let env_struct = format!("{}_env_t", Self::func_c_name(func_name));
                    let env_tmp = self.fresh_tmp();
                    parts.push(format!(
                        "{}* {} = ({}*)ori_alloc(sizeof({}), 0)",
                        env_struct, env_tmp, env_struct, env_struct
                    ));
                    for cap in captures {
                        let cap_s = mangle(&cap.name);
                        parts.push(format!("{}->{} = {}", env_tmp, cap_s, cap_s));
                        if let Some(child_access) =
                            c_arc_access(&format!("{}->{}", env_tmp, cap_s), &cap.ty)
                        {
                            parts.push(format!(
                                "ori_arc_register_edge((void*){}, {})",
                                env_tmp, child_access
                            ));
                        }
                    }
                    parts.push(format!("{}->env_ptr = (void*){}", tmp, env_tmp));
                    parts.push(format!(
                        "ori_arc_register_edge((void*){}, (void*){})",
                        tmp, env_tmp
                    ));
                    parts.push(format!("ori_arc_release((void*){})", env_tmp));
                }
                format!("({{ {}; {}; }})", parts.join("; "), tmp)
            }
        }
    }

    fn equality_to_c(&mut self, left: String, right: String, ty: &Ty, eq: bool) -> String {
        let equal = match ty {
            Ty::String => format!("ori_string_eq({}, {})", left, right),
            Ty::Any(_) => self.any_equality_to_c(left, right),
            Ty::Opaque { kind, args } if kind.is_list_backed_collection() => {
                self.opaque_collection_equality_to_c(left, right, *kind, &args[0])
            }
            Ty::List(inner) => self.list_equality_to_c(left, right, inner),
            Ty::Optional(inner) => self.optional_equality_to_c(left, right, inner),
            Ty::Result(ok, err) => self.result_equality_to_c(left, right, ok, err),
            Ty::Tuple(elements) => self.tuple_equality_to_c(left, right, elements),
            Ty::Set(inner) => self.set_equality_to_c(left, right, inner),
            Ty::Map(key, value) => self.map_equality_to_c(left, right, key, value),
            Ty::Named(def_id, args) => self.struct_equality_to_c(left, right, *def_id, args),
            _ if ty.is_numeric() || matches!(ty, Ty::Bool) => format!("({} == {})", left, right),
            _ => format!("({} == {})", left, right),
        };
        if eq {
            equal
        } else {
            format!("(!{})", equal)
        }
    }

    fn any_equality_to_c(&mut self, left: String, right: String) -> String {
        let l_tmp = self.fresh_tmp();
        let r_tmp = self.fresh_tmp();
        let res_tmp = self.fresh_tmp();
        format!(
            "({{ ori_any_t {l_tmp} = {left}; ori_any_t {r_tmp} = {right}; bool {res_tmp} = false; \
             if (((void**){l_tmp}.vtable)[0] == ((void**){r_tmp}.vtable)[0]) {{ \
                 bool (*eq_fn)(void*, void*) = (bool (*)(void*, void*))((void**){l_tmp}.vtable)[1]; \
                 if (eq_fn) {{ \
                     {res_tmp} = eq_fn({l_tmp}.obj, {r_tmp}.obj); \
                 }} \
             }} \
             {res_tmp}; }})"
        )
    }

    fn opaque_collection_equality_to_c(
        &mut self,
        left: String,
        right: String,
        kind: OpaqueTy,
        elem_ty: &Ty,
    ) -> String {
        let to_list_fn = match kind {
            OpaqueTy::Deque => "ori_deque_to_list",
            OpaqueTy::Queue => "ori_queue_to_list",
            OpaqueTy::Stack => "ori_stack_to_list",
            OpaqueTy::LinkedList => "ori_linked_list_to_list",
            OpaqueTy::DoublyLinkedList => "ori_doubly_linked_list_to_list",
            _ => panic!("unsupported opaque collection for C equality"),
        };
        let l_tmp = self.fresh_tmp();
        let r_tmp = self.fresh_tmp();
        let l_list = self.fresh_tmp();
        let r_list = self.fresh_tmp();
        let res_tmp = self.fresh_tmp();

        let list_eq = self.list_equality_to_c(l_list.clone(), r_list.clone(), elem_ty);

        format!(
            "({{ void* {l_tmp} = {left}; void* {r_tmp} = {right}; \
             void* {l_list} = {to_list_fn}({l_tmp}); \
             void* {r_list} = {to_list_fn}({r_tmp}); \
             bool {res_tmp} = {list_eq}; \
             ori_arc_release({l_list}); \
             ori_arc_release({r_list}); \
             {res_tmp}; }})"
        )
    }

    fn list_equality_to_c(&mut self, left: String, right: String, inner: &Ty) -> String {
        let left_tmp = self.fresh_tmp();
        let right_tmp = self.fresh_tmp();
        let index_tmp = self.fresh_tmp();
        let same_tmp = self.fresh_tmp();
        let elem_c = ty_to_c(inner);
        let left_elem = format!(
            "*(({}*)ori_list_at(&{}, (int64_t){}))",
            elem_c, left_tmp, index_tmp
        );
        let right_elem = format!(
            "*(({}*)ori_list_at(&{}, (int64_t){}))",
            elem_c, right_tmp, index_tmp
        );
        let elem_equal = self.equality_to_c(left_elem, right_elem, inner, true);
        format!(
            "({{ ori_list_t {left_tmp} = {left}; ori_list_t {right_tmp} = {right}; bool {same_tmp} = {left_tmp}.len == {right_tmp}.len; if ({same_tmp}) {{ for (size_t {index_tmp} = 0; {index_tmp} < {left_tmp}.len; {index_tmp}++) {{ if (!({elem_equal})) {{ {same_tmp} = false; break; }} }} }} {same_tmp}; }})"
        )
    }

    fn set_equality_to_c(&mut self, left: String, right: String, inner: &Ty) -> String {
        if !matches!(inner, Ty::Int | Ty::String) {
            return self.unsupported_expr(format!(
                "C backend cannot lower set structural equality for `{}` keys yet",
                inner.display()
            ));
        }
        let left_tmp = self.fresh_tmp();
        let right_tmp = self.fresh_tmp();
        let index_tmp = self.fresh_tmp();
        let same_tmp = self.fresh_tmp();
        let value_at = if matches!(inner, Ty::String) {
            "ori_set_string_at"
        } else {
            "ori_set_value_at"
        };
        let contains = if matches!(inner, Ty::String) {
            "ori_set_contains_string"
        } else {
            "ori_set_contains"
        };
        format!(
            "({{ ori_set_t* {left_tmp} = {left}; ori_set_t* {right_tmp} = {right}; bool {same_tmp} = ori_set_len({left_tmp}) == ori_set_len({right_tmp}); if ({same_tmp}) {{ for (int64_t {index_tmp} = 0; {index_tmp} < ori_set_len({left_tmp}); {index_tmp}++) {{ if (!{contains}({right_tmp}, {value_at}({left_tmp}, {index_tmp}))) {{ {same_tmp} = false; break; }} }} }} {same_tmp}; }})"
        )
    }

    fn map_equality_to_c(
        &mut self,
        left: String,
        right: String,
        key_ty: &Ty,
        value_ty: &Ty,
    ) -> String {
        if !matches!(key_ty, Ty::Int | Ty::String) {
            return self.unsupported_expr(format!(
                "C backend cannot lower map structural equality for `{}` keys yet",
                key_ty.display()
            ));
        }
        let left_tmp = self.fresh_tmp();
        let right_tmp = self.fresh_tmp();
        let index_tmp = self.fresh_tmp();
        let same_tmp = self.fresh_tmp();
        let key_tmp = self.fresh_tmp();
        let left_value_tmp = self.fresh_tmp();
        let right_value_tmp = self.fresh_tmp();
        let key_c = ty_to_c(key_ty);
        let key_at = if matches!(key_ty, Ty::String) {
            "ori_map_key_string_at"
        } else {
            "ori_map_key_at"
        };
        let contains = if matches!(key_ty, Ty::String) {
            "ori_map_contains_string"
        } else {
            "ori_map_contains"
        };
        let get_value = if matches!(key_ty, Ty::String) {
            "ori_map_get_string_value"
        } else {
            "ori_map_get_value"
        };
        let value_c = ty_to_c(value_ty);
        let value_equal = self.equality_to_c(
            format!("*(({value_c}*){left_value_tmp})"),
            format!("*(({value_c}*){right_value_tmp})"),
            value_ty,
            true,
        );
        format!(
            "({{ ori_map_t* {left_tmp} = {left}; ori_map_t* {right_tmp} = {right}; bool {same_tmp} = ori_map_len({left_tmp}) == ori_map_len({right_tmp}); if ({same_tmp}) {{ for (int64_t {index_tmp} = 0; {index_tmp} < ori_map_len({left_tmp}); {index_tmp}++) {{ {key_c} {key_tmp} = {key_at}({left_tmp}, {index_tmp}); if (!{contains}({right_tmp}, {key_tmp})) {{ {same_tmp} = false; break; }} void* {left_value_tmp} = ori_map_value_at({left_tmp}, {index_tmp}); void* {right_value_tmp} = {get_value}({right_tmp}, {key_tmp}); if (!{left_value_tmp} || !{right_value_tmp} || !({value_equal})) {{ {same_tmp} = false; break; }} }} }} {same_tmp}; }})"
        )
    }

    fn optional_equality_to_c(&mut self, left: String, right: String, inner: &Ty) -> String {
        let left_tmp = self.fresh_tmp();
        let right_tmp = self.fresh_tmp();
        let same_tmp = self.fresh_tmp();
        let ty_c = ty_to_c(&Ty::Optional(Box::new(inner.clone())));
        let value_equal = self.equality_to_c(
            format!("{left_tmp}.value"),
            format!("{right_tmp}.value"),
            inner,
            true,
        );
        format!(
            "({{ {ty_c} {left_tmp} = {left}; {ty_c} {right_tmp} = {right}; bool {same_tmp} = {left_tmp}.has_value == {right_tmp}.has_value; if ({same_tmp} && {left_tmp}.has_value) {{ {same_tmp} = {value_equal}; }} {same_tmp}; }})"
        )
    }

    fn result_equality_to_c(&mut self, left: String, right: String, ok: &Ty, err: &Ty) -> String {
        let left_tmp = self.fresh_tmp();
        let right_tmp = self.fresh_tmp();
        let same_tmp = self.fresh_tmp();
        let ty_c = ty_to_c(&Ty::Result(Box::new(ok.clone()), Box::new(err.clone())));
        let ok_equal = self.equality_to_c(
            format!("{left_tmp}.value.ok"),
            format!("{right_tmp}.value.ok"),
            ok,
            true,
        );
        let err_equal = self.equality_to_c(
            format!("{left_tmp}.value.err"),
            format!("{right_tmp}.value.err"),
            err,
            true,
        );
        format!(
            "({{ {ty_c} {left_tmp} = {left}; {ty_c} {right_tmp} = {right}; bool {same_tmp} = {left_tmp}.is_ok == {right_tmp}.is_ok; if ({same_tmp}) {{ {same_tmp} = {left_tmp}.is_ok ? ({ok_equal}) : ({err_equal}); }} {same_tmp}; }})"
        )
    }

    fn tuple_equality_to_c(&mut self, left: String, right: String, elements: &[Ty]) -> String {
        let left_tmp = self.fresh_tmp();
        let right_tmp = self.fresh_tmp();
        let same_tmp = self.fresh_tmp();
        let ty_c = ty_to_c(&Ty::Tuple(elements.to_vec()));
        let mut checks = String::new();
        for (index, elem) in elements.iter().enumerate() {
            let equal = self.equality_to_c(
                format!("{left_tmp}._f{index}"),
                format!("{right_tmp}._f{index}"),
                elem,
                true,
            );
            checks.push_str(&format!(" if ({same_tmp}) {{ {same_tmp} = {equal}; }}"));
        }
        format!(
            "({{ {ty_c} {left_tmp} = {left}; {ty_c} {right_tmp} = {right}; bool {same_tmp} = true;{checks} {same_tmp}; }})"
        )
    }

    fn struct_equality_to_c(
        &mut self,
        left: String,
        right: String,
        def_id: DefId,
        args: &[Ty],
    ) -> String {
        let Some(fields) = self.struct_fields.get(&def_id).cloned() else {
            return self.unsupported_expr(
                "C backend cannot lower non-struct user-defined structural equality",
            );
        };
        let left_tmp = self.fresh_tmp();
        let right_tmp = self.fresh_tmp();
        let same_tmp = self.fresh_tmp();
        let ty_c = def_c_name(def_id);
        let mut checks = String::new();
        for (name, ty) in fields {
            let concrete_field_ty = substitute_ty_params(&ty, args);
            let name = mangle(&name);
            let equal = self.equality_to_c(
                format!("{left_tmp}.{name}"),
                format!("{right_tmp}.{name}"),
                &concrete_field_ty,
                true,
            );
            checks.push_str(&format!(" if ({same_tmp}) {{ {same_tmp} = {equal}; }}"));
        }
        format!(
            "({{ {ty_c} {left_tmp} = {left}; {ty_c} {right_tmp} = {right}; bool {same_tmp} = true;{checks} {same_tmp}; }})"
        )
    }

    fn emit_lazy_once(&mut self, thunk: &HirExpr, lazy_ty: &Ty) -> String {
        let Ty::Lazy(inner) = lazy_ty else {
            return self.unsupported_expr(format!(
                "C backend cannot create lazy value with non-lazy type `{}`",
                lazy_ty.display()
            ));
        };
        let thunk_s = self.expr_to_c(thunk);
        let tmp = self.fresh_tmp();
        let lazy_ptr_ty = ty_to_c(lazy_ty);
        let lazy_struct_ty = lazy_c_name(inner);
        format!(
            "(void*)({{ {lazy_ptr_ty} {tmp} = ({lazy_ptr_ty})ori_alloc(sizeof({lazy_struct_ty}), 0); {tmp}->thunk = {thunk_s}; {tmp}->forced = false; memset(&{tmp}->value, 0, sizeof({tmp}->value)); ori_arc_register_edge((void*){tmp}, (void*){tmp}->thunk); ori_arc_release((void*){tmp}->thunk); {tmp}; }})"
        )
    }

    fn emit_lazy_force(&mut self, value: &HirExpr, _ret_ty: &Ty) -> String {
        let Ty::Lazy(inner) = &value.ty else {
            return self.unsupported_expr(format!(
                "C backend cannot force non-lazy type `{}`",
                value.ty.display()
            ));
        };
        if matches!(inner.as_ref(), Ty::Void | Ty::Never) {
            return self.unsupported_expr("C backend cannot force lazy<void> values yet");
        }
        let lazy_s = self.expr_to_c(value);
        let tmp = self.fresh_tmp();
        let lazy_ptr_ty = ty_to_c(&value.ty);
        let ret_c = ty_to_c(inner);
        let fn_cast = format!("(({} (*)(void*)){}->thunk->fn_ptr)", ret_c, tmp);
        let mut force_body = vec![
            format!("{tmp}->value = {fn_cast}({tmp}->thunk->env_ptr)"),
            format!("{tmp}->forced = true"),
        ];
        if let Some(access) = c_arc_access(&format!("{tmp}->value"), inner) {
            force_body.push(format!("ori_arc_register_edge((void*){tmp}, {access})"));
            force_body.push(format!("ori_arc_release({access})"));
        }
        format!(
            "({{ {lazy_ptr_ty} {tmp} = {lazy_s}; if (!{tmp}->forced) {{ {}; }} {tmp}->value; }})",
            force_body.join("; ")
        )
    }

    fn emit_builtin_or(&mut self, value: &HirExpr, fallback: &HirExpr) -> String {
        let value_s = self.expr_to_c(value);
        let tmp = self.fresh_tmp();
        match &value.ty {
            Ty::Optional(inner) => {
                let fallback_s = self.expr_to_c_for_expected(fallback, inner);
                format!(
                    "({{ {} {} = {}; {}.has_value ? {}.value : {}; }})",
                    ty_to_c(&value.ty),
                    tmp,
                    value_s,
                    tmp,
                    tmp,
                    fallback_s
                )
            }
            Ty::Result(ok, _) => {
                let fallback_s = self.expr_to_c_for_expected(fallback, ok);
                format!(
                    "({{ {} {} = {}; {}.is_ok ? {}.value.ok : {}; }})",
                    ty_to_c(&value.ty),
                    tmp,
                    value_s,
                    tmp,
                    tmp,
                    fallback_s
                )
            }
            other => self.unsupported_expr(format!(
                "C backend cannot lower `.or()` for `{}`",
                other.display()
            )),
        }
    }

    fn emit_builtin_or_wrap(&mut self, value: &HirExpr, context: &HirExpr) -> String {
        let value_s = self.expr_to_c(value);
        let tmp = self.fresh_tmp();
        let context_tmp = self.fresh_tmp();
        let prefix_tmp = self.fresh_tmp();
        let context_s = self.expr_to_c_for_expected(context, &Ty::String);
        match &value.ty {
            Ty::Result(_, err) if matches!(**err, Ty::String) => format!(
                "({{ {} {} = {}; if (!{}.is_ok) {{ ori_string_t {} = {}; ori_string_t {} = ori_string_concat({}, ORI_STR(\": \")); {}.value.err = ori_string_concat({}, {}.value.err); }} {}; }})",
                ty_to_c(&value.ty),
                tmp,
                value_s,
                tmp,
                context_tmp,
                context_s,
                prefix_tmp,
                context_tmp,
                tmp,
                prefix_tmp,
                tmp,
                tmp
            ),
            other => self.unsupported_expr(format!(
                "C backend cannot lower `.or_wrap()` for `{}`",
                other.display()
            )),
        }
    }

    fn emit_map_runtime_call(&mut self, name: &str, args: &[HirArg], ret_ty: &Ty) -> String {
        let Some(first_arg) = args.first() else {
            return self
                .unsupported_expr(format!("C backend map runtime call `{name}` has no map"));
        };
        let Ty::Map(key_ty, value_ty) = &first_arg.value.ty else {
            return self.unsupported_expr(format!(
                "C backend map runtime call `{name}` received `{}`",
                first_arg.value.ty.display()
            ));
        };
        let key_ty = key_ty.as_ref();
        let value_ty = value_ty.as_ref();
        if !matches!(key_ty, Ty::Int | Ty::String) {
            return self.unsupported_expr(format!(
                "C backend map runtime call `{name}` does not support `{}` keys yet",
                key_ty.display()
            ));
        }
        let map_s = self.expr_to_c(&first_arg.value);

        match name {
            "ori_map_set" => {
                if args.len() != 3 {
                    return self.unsupported_expr("C backend map.set expects map, key and value");
                }
                let key_s = self.map_key_to_c(&args[1].value, key_ty);
                let value_s = self.expr_to_c_for_expected(&args[2].value, value_ty);
                let value_tmp = self.fresh_tmp();
                let value_c = ty_to_c(value_ty);
                let runtime_name = if matches!(key_ty, Ty::String) {
                    "ori_map_set_string_value"
                } else {
                    "ori_map_set_value"
                };
                format!(
                    "({{ {value_c} {value_tmp} = {value_s}; {runtime_name}({map_s}, {key_s}, &{value_tmp}, sizeof({value_tmp})); }})"
                )
            }
            "ori_map_get" => {
                if args.len() != 2 {
                    return self.unsupported_expr("C backend map.get expects map and key");
                }
                let key_s = self.map_key_to_c(&args[1].value, key_ty);
                let value_c = ty_to_c(value_ty);
                let value_tmp = self.fresh_tmp();
                let runtime_name = if matches!(key_ty, Ty::String) {
                    "ori_map_get_string_value"
                } else {
                    "ori_map_get_value"
                };
                format!(
                    "({{ void* {value_tmp} = {runtime_name}({map_s}, {key_s}); {value_tmp} ? *(({value_c}*){value_tmp}) : {}; }})",
                    c_default_value(value_ty)
                )
            }
            "ori_map_contains" => {
                if args.len() != 2 {
                    return self.unsupported_expr("C backend map.contains expects map and key");
                }
                let key_s = self.map_key_to_c(&args[1].value, key_ty);
                let runtime_name = if matches!(key_ty, Ty::String) {
                    "ori_map_contains_string"
                } else {
                    "ori_map_contains"
                };
                format!("{runtime_name}({map_s}, {key_s})")
            }
            "ori_map_remove" | "ori_map_try_remove" | "ori_map_try_get" => {
                if args.len() != 2 {
                    return self.unsupported_expr(format!("C backend {name} expects map and key"));
                }
                let key_s = self.map_key_to_c(&args[1].value, key_ty);
                let runtime_name = match (name, key_ty) {
                    ("ori_map_remove", Ty::String) => "ori_map_remove_string",
                    ("ori_map_try_remove", Ty::String) => "ori_map_try_remove_string",
                    ("ori_map_try_get", Ty::String) => "ori_map_try_get_string",
                    _ => name,
                };
                let call = format!("{runtime_name}({map_s}, {key_s})");
                if matches!(ret_ty, Ty::Void) {
                    format!("({{ {call}; }})")
                } else {
                    call
                }
            }
            _ => self.unsupported_expr(format!("C backend map runtime call `{name}`")),
        }
    }

    fn emit_set_runtime_call(&mut self, name: &str, args: &[HirArg], ret_ty: &Ty) -> String {
        let Some(first_arg) = args.first() else {
            return self
                .unsupported_expr(format!("C backend set runtime call `{name}` has no set"));
        };
        let Ty::Set(elem_ty) = &first_arg.value.ty else {
            return self.unsupported_expr(format!(
                "C backend set runtime call `{name}` received `{}`",
                first_arg.value.ty.display()
            ));
        };
        let elem_ty = elem_ty.as_ref();
        if !matches!(elem_ty, Ty::Int | Ty::String) {
            return self.unsupported_expr(format!(
                "C backend set runtime call `{name}` does not support `{}` values yet",
                elem_ty.display()
            ));
        }
        let set_s = self.expr_to_c(&first_arg.value);

        match name {
            "ori_set_add" => {
                if args.len() != 2 {
                    return self.unsupported_expr("C backend set.add expects set and value");
                }
                let value_s = self.set_key_to_c(&args[1].value, elem_ty);
                let runtime_name = if matches!(elem_ty, Ty::String) {
                    "ori_set_add_string"
                } else {
                    "ori_set_add"
                };
                format!("({{ {runtime_name}({set_s}, {value_s}); }})")
            }
            "ori_set_contains" => {
                if args.len() != 2 {
                    return self.unsupported_expr("C backend set.contains expects set and value");
                }
                let value_s = self.set_key_to_c(&args[1].value, elem_ty);
                let runtime_name = if matches!(elem_ty, Ty::String) {
                    "ori_set_contains_string"
                } else {
                    "ori_set_contains"
                };
                format!("{runtime_name}({set_s}, {value_s})")
            }
            "ori_set_remove" | "ori_set_try_remove" => {
                if args.len() != 2 {
                    return self
                        .unsupported_expr(format!("C backend {name} expects set and value"));
                }
                let value_s = self.set_key_to_c(&args[1].value, elem_ty);
                let runtime_name = match (name, elem_ty) {
                    ("ori_set_remove", Ty::String) => "ori_set_remove_string",
                    ("ori_set_try_remove", Ty::String) => "ori_set_try_remove_string",
                    _ => name,
                };
                let call = format!("{runtime_name}({set_s}, {value_s})");
                if matches!(ret_ty, Ty::Void) {
                    format!("({{ {call}; }})")
                } else {
                    call
                }
            }
            _ => self.unsupported_expr(format!("C backend set runtime call `{name}`")),
        }
    }

    fn map_key_to_c(&mut self, expr: &HirExpr, key_ty: &Ty) -> String {
        let key_s = self.expr_to_c_for_expected(expr, key_ty);
        if matches!(key_ty, Ty::String) {
            key_s
        } else {
            format!("(int64_t)({key_s})")
        }
    }

    fn set_key_to_c(&mut self, expr: &HirExpr, elem_ty: &Ty) -> String {
        let value_s = self.expr_to_c_for_expected(expr, elem_ty);
        if matches!(elem_ty, Ty::String) {
            value_s
        } else {
            format!("(int64_t)({value_s})")
        }
    }

    fn emit_string_parse_int(&mut self, input: &str, result_ty: &Ty) -> String {
        let string_tmp = self.fresh_tmp();
        let buf_tmp = self.fresh_tmp();
        let end_tmp = self.fresh_tmp();
        let value_tmp = self.fresh_tmp();
        let result_tmp = self.fresh_tmp();
        let result_c = ty_to_c(result_ty);
        format!(
            "({{ ori_string_t {string_tmp} = {input}; char* {buf_tmp} = (char*)malloc({string_tmp}.len + 1); memcpy({buf_tmp}, {string_tmp}.data, {string_tmp}.len); {buf_tmp}[{string_tmp}.len] = '\\0'; char* {end_tmp} = NULL; int64_t {value_tmp} = strtoll({buf_tmp}, &{end_tmp}, 10); while ({end_tmp} && *{end_tmp} && isspace((unsigned char)*{end_tmp})) {end_tmp}++; {result_c} {result_tmp}; if ({end_tmp} && {end_tmp} != {buf_tmp} && *{end_tmp} == '\\0') {{ {result_tmp} = (({result_c}){{ .is_ok = true, .value.ok = {value_tmp} }}); }} else {{ {result_tmp} = (({result_c}){{ .is_ok = false, .value.err = ORI_STR(\"invalid int\") }}); }} free({buf_tmp}); {result_tmp}; }})"
        )
    }

    fn emit_string_parse_float(&mut self, input: &str, result_ty: &Ty) -> String {
        let string_tmp = self.fresh_tmp();
        let buf_tmp = self.fresh_tmp();
        let end_tmp = self.fresh_tmp();
        let value_tmp = self.fresh_tmp();
        let result_tmp = self.fresh_tmp();
        let result_c = ty_to_c(result_ty);
        format!(
            "({{ ori_string_t {string_tmp} = {input}; char* {buf_tmp} = (char*)malloc({string_tmp}.len + 1); memcpy({buf_tmp}, {string_tmp}.data, {string_tmp}.len); {buf_tmp}[{string_tmp}.len] = '\\0'; char* {end_tmp} = NULL; double {value_tmp} = strtod({buf_tmp}, &{end_tmp}); while ({end_tmp} && *{end_tmp} && isspace((unsigned char)*{end_tmp})) {end_tmp}++; {result_c} {result_tmp}; if ({end_tmp} && {end_tmp} != {buf_tmp} && *{end_tmp} == '\\0') {{ {result_tmp} = (({result_c}){{ .is_ok = true, .value.ok = {value_tmp} }}); }} else {{ {result_tmp} = (({result_c}){{ .is_ok = false, .value.err = ORI_STR(\"invalid float\") }}); }} free({buf_tmp}); {result_tmp}; }})"
        )
    }

    /// Emit string interpolation: `f"hello {name}, age {age}"`
    /// Strategy: build with snprintf into a heap buffer.
    fn emit_interp_str(&mut self, parts: &[HirStrPart]) -> String {
        let mut fmt = String::new();
        let mut args: Vec<String> = Vec::new();
        let mut prelude: Vec<String> = Vec::new();
        for part in parts {
            match part {
                HirStrPart::Literal(s) => {
                    fmt.push_str(&escape_c_str(s));
                }
                HirStrPart::Expr(e) => {
                    let val = self.expr_to_c(e);
                    match &e.ty {
                        Ty::Int | Ty::Int8 | Ty::Int16 | Ty::Int32 | Ty::Int64 => {
                            let tmp = self.fresh_tmp();
                            // Use PRId64 via C string concatenation in the emitted format
                            fmt.push_str("%\" PRId64 \"");
                            prelude.push(format!("int64_t {tmp} = (int64_t)({val});"));
                            args.push(tmp);
                        }
                        Ty::Float | Ty::Float32 | Ty::Float64 => {
                            let tmp = self.fresh_tmp();
                            fmt.push_str("%g");
                            prelude.push(format!("double {tmp} = (double)({val});"));
                            args.push(tmp);
                        }
                        Ty::Bool => {
                            let tmp = self.fresh_tmp();
                            fmt.push_str("%s");
                            prelude.push(format!("bool {tmp} = (bool)({val});"));
                            args.push(format!("({tmp} ? \"true\" : \"false\")"));
                        }
                        Ty::String => {
                            let tmp = self.fresh_tmp();
                            fmt.push_str("%.*s");
                            prelude.push(format!("ori_string_t {tmp} = {val};"));
                            args.push(format!("(int){tmp}.len, {tmp}.data"));
                        }
                        _ => {
                            let tmp = self.fresh_tmp();
                            fmt.push_str("%.*s");
                            prelude.push(format!("ori_string_t {tmp} = {val};"));
                            args.push(format!("(int){tmp}.len, {tmp}.data"));
                        }
                    }
                }
            }
        }
        let tmp_buf = self.fresh_tmp();
        let tmp_len = self.fresh_tmp();
        let prelude = if prelude.is_empty() {
            String::new()
        } else {
            format!("{} ", prelude.join(" "))
        };
        let args_str = if args.is_empty() {
            String::new()
        } else {
            format!(", {}", args.join(", "))
        };
        format!(
            "({{ {prelude}int {len} = snprintf(NULL, 0, \"{fmt}\"{args}); char* {buf} = (char*)malloc((size_t){len} + 1); snprintf({buf}, (size_t){len} + 1, \"{fmt}\"{args}); (ori_string_t){{ .data = {buf}, .len = (size_t){len} }}; }})",
            prelude = prelude,
            buf = tmp_buf,
            len = tmp_len,
            fmt = fmt,
            args = args_str,
        )
    }

    fn lvalue_to_c(&mut self, lv: &HirLValue) -> String {
        match lv {
            HirLValue::Var(n) => mangle(n),
            HirLValue::Field { base, field } => {
                format!("{}.{}", self.lvalue_to_c(base), mangle(field))
            }
            HirLValue::Index { base, index } => {
                let idx_s = self.lvalue_index_to_c(index);
                format!("{}[{}]", self.lvalue_to_c(base), idx_s)
            }
        }
    }

    fn lvalue_index_to_c(&mut self, expr: &HirExpr) -> String {
        match &expr.kind {
            HirExprKind::IntLit(n) => format!("INT64_C({})", n),
            HirExprKind::Var(n) => mangle(n),
            HirExprKind::BoolLit(b) => {
                if *b {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            HirExprKind::FloatLit(f) => float_lit_to_c(*f),
            HirExprKind::Binary { op, lhs, rhs } => {
                let l = self.lvalue_index_to_c(lhs);
                let r = self.lvalue_index_to_c(rhs);
                format!("({} {} {})", l, binop_to_c(*op), r)
            }
            HirExprKind::Unary { op, operand } => {
                let e = self.lvalue_index_to_c(operand);
                match op {
                    UnaryOp::Neg => format!("(-{})", e),
                    UnaryOp::Not => format!("(!{})", e),
                }
            }
            HirExprKind::Field { object, field } => {
                let obj = self.lvalue_index_to_c(object);
                format!("{}.{}", obj, mangle(field))
            }
            _ => self.unsupported_expr(format!(
                "C backend does not support `{}` as lvalue index expression",
                expr.ty.display()
            )),
        }
    }

    fn managed_local_ty(&self, name: &str) -> Option<&Ty> {
        self.managed_stack
            .iter()
            .rev()
            .find_map(|(local_name, ty)| (local_name == name).then_some(ty))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn def_c_name(id: DefId) -> String {
    format!("ori_def_{}_t", id.0)
}

fn collect_abi_types(module: &HirModule) -> Vec<Ty> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for s in &module.structs {
        for field in &s.fields {
            collect_ty_abi(&field.ty, &mut seen, &mut out);
        }
    }
    for e in &module.enums {
        for variant in &e.variants {
            for field in &variant.fields {
                collect_ty_abi(&field.ty, &mut seen, &mut out);
            }
        }
    }
    for c in &module.consts {
        collect_ty_abi(&c.ty, &mut seen, &mut out);
        collect_expr_abi(&c.value, &mut seen, &mut out);
    }
    for f in &module.funcs {
        collect_ty_abi(&f.return_ty, &mut seen, &mut out);
        for param in &f.params {
            collect_ty_abi(&param.ty, &mut seen, &mut out);
        }
        collect_block_abi(&f.body, &mut seen, &mut out);
    }
    for ext in &module.externs {
        match ext {
            HirExtern::Func {
                params, return_ty, ..
            } => {
                collect_ty_abi(return_ty, &mut seen, &mut out);
                for param in params {
                    collect_ty_abi(&param.ty, &mut seen, &mut out);
                }
            }
            HirExtern::Var { ty, .. } => collect_ty_abi(ty, &mut seen, &mut out),
        }
    }
    out
}

fn collect_block_abi(block: &HirBlock, seen: &mut HashSet<String>, out: &mut Vec<Ty>) {
    for stmt in &block.stmts {
        collect_stmt_abi(stmt, seen, out);
    }
}

fn collect_stmt_abi(stmt: &HirStmt, seen: &mut HashSet<String>, out: &mut Vec<Ty>) {
    match stmt {
        HirStmt::Let { ty, value, .. } => {
            collect_ty_abi(ty, seen, out);
            collect_expr_abi(value, seen, out);
        }
        HirStmt::Assign { value, .. } | HirStmt::Expr(value) => collect_expr_abi(value, seen, out),
        HirStmt::Return(Some(value), _) => collect_expr_abi(value, seen, out),
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            collect_expr_abi(cond, seen, out);
            collect_block_abi(then, seen, out);
            for (cond, block) in else_ifs {
                collect_expr_abi(cond, seen, out);
                collect_block_abi(block, seen, out);
            }
            if let Some(block) = else_ {
                collect_block_abi(block, seen, out);
            }
        }
        HirStmt::While { cond, body, .. } => {
            collect_expr_abi(cond, seen, out);
            collect_block_abi(body, seen, out);
        }
        HirStmt::For {
            elem_ty,
            iterable,
            body,
            ..
        } => {
            collect_ty_abi(elem_ty, seen, out);
            collect_expr_abi(iterable, seen, out);
            collect_block_abi(body, seen, out);
        }
        HirStmt::Loop { body, .. } => collect_block_abi(body, seen, out),
        HirStmt::Repeat { count, body, .. } => {
            collect_expr_abi(count, seen, out);
            collect_block_abi(body, seen, out);
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            collect_expr_abi(scrutinee, seen, out);
            for arm in arms {
                collect_pattern_abi(&arm.pattern, seen, out);
                if let Some(guard) = &arm.guard {
                    collect_expr_abi(guard, seen, out);
                }
                for stmt in &arm.body {
                    collect_stmt_abi(stmt, seen, out);
                }
            }
        }
        HirStmt::IfSome {
            inner_ty,
            value,
            then,
            else_,
            ..
        } => {
            collect_ty_abi(inner_ty, seen, out);
            collect_expr_abi(value, seen, out);
            collect_block_abi(then, seen, out);
            if let Some(block) = else_ {
                collect_block_abi(block, seen, out);
            }
        }
        HirStmt::WhileSome {
            inner_ty,
            value,
            body,
            ..
        } => {
            collect_ty_abi(inner_ty, seen, out);
            collect_expr_abi(value, seen, out);
            collect_block_abi(body, seen, out);
        }
        HirStmt::Using { ty, value, .. } => {
            collect_ty_abi(ty, seen, out);
            collect_expr_abi(value, seen, out);
        }
        HirStmt::Check { condition, .. } => collect_expr_abi(condition, seen, out),
    }
}

fn collect_pattern_abi(pattern: &HirPattern, seen: &mut HashSet<String>, out: &mut Vec<Ty>) {
    match pattern {
        HirPattern::Binding(_, ty) => collect_ty_abi(ty, seen, out),
        HirPattern::Some_(inner) | HirPattern::Ok_(inner) | HirPattern::Err_(inner) => {
            collect_pattern_abi(inner, seen, out);
        }
        HirPattern::Variant { fields, .. } => {
            for (_, pattern) in fields {
                collect_pattern_abi(pattern, seen, out);
            }
        }
        HirPattern::Tuple(items) => {
            for item in items {
                collect_pattern_abi(item, seen, out);
            }
        }
        _ => {}
    }
}

fn collect_expr_abi(expr: &HirExpr, seen: &mut HashSet<String>, out: &mut Vec<Ty>) {
    collect_ty_abi(&expr.ty, seen, out);
    match &expr.kind {
        HirExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_abi(lhs, seen, out);
            collect_expr_abi(rhs, seen, out);
        }
        HirExprKind::Unary { operand, .. }
        | HirExprKind::Some_(operand)
        | HirExprKind::Ok_(operand)
        | HirExprKind::Err_(operand)
        | HirExprKind::Propagate(operand)
        | HirExprKind::Await(operand) => collect_expr_abi(operand, seen, out),
        HirExprKind::Field { object, .. } | HirExprKind::TupleIndex { object, .. } => {
            collect_expr_abi(object, seen, out);
        }
        HirExprKind::Index { object, index } => {
            collect_expr_abi(object, seen, out);
            collect_expr_abi(index, seen, out);
        }
        HirExprKind::Call { callee, args } => {
            collect_expr_abi(callee, seen, out);
            for arg in args {
                collect_expr_abi(&arg.value, seen, out);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            collect_expr_abi(receiver, seen, out);
            for arg in args {
                collect_expr_abi(arg, seen, out);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, expr) in fields {
                collect_expr_abi(expr, seen, out);
            }
        }
        HirExprKind::ListLit { elem_ty, elements } => {
            collect_ty_abi(elem_ty, seen, out);
            for expr in elements {
                collect_expr_abi(expr, seen, out);
            }
        }
        HirExprKind::ListSpreadLit { elem_ty, elements } => {
            collect_ty_abi(elem_ty, seen, out);
            for elem in elements {
                collect_expr_abi(&elem.value, seen, out);
            }
        }
        HirExprKind::TupleLit(items) => {
            for item in items {
                collect_expr_abi(item, seen, out);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for part in parts {
                if let HirStrPart::Expr(expr) = part {
                    collect_expr_abi(expr, seen, out);
                }
            }
        }
        HirExprKind::Range { start, end } => {
            collect_expr_abi(start, seen, out);
            collect_expr_abi(end, seen, out);
        }
        HirExprKind::MapLit {
            key_ty,
            value_ty,
            entries,
        } => {
            collect_ty_abi(key_ty, seen, out);
            collect_ty_abi(value_ty, seen, out);
            for (k, v) in entries {
                collect_expr_abi(k, seen, out);
                collect_expr_abi(v, seen, out);
            }
        }
        HirExprKind::SetLit { elem_ty, elements } => {
            collect_ty_abi(elem_ty, seen, out);
            for e in elements {
                collect_expr_abi(e, seen, out);
            }
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            collect_expr_abi(base, seen, out);
            for (_, e) in updates {
                collect_expr_abi(e, seen, out);
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            collect_expr_abi(cond, seen, out);
            collect_expr_abi(then, seen, out);
            collect_expr_abi(else_, seen, out);
        }
        HirExprKind::IsCheck { value, .. } => {
            collect_expr_abi(value, seen, out);
        }
        _ => {}
    }
}

fn collect_ty_abi(ty: &Ty, seen: &mut HashSet<String>, out: &mut Vec<Ty>) {
    match ty {
        Ty::Optional(inner) => {
            collect_ty_abi(inner, seen, out);
            push_abi_ty(ty, seen, out);
        }
        Ty::Result(ok, err) => {
            collect_ty_abi(ok, seen, out);
            collect_ty_abi(err, seen, out);
            push_abi_ty(ty, seen, out);
        }
        Ty::List(inner)
        | Ty::Set(inner)
        | Ty::Range(inner)
        | Ty::Future(inner)
        | Ty::TaskJob(inner)
        | Ty::Channel(inner) => {
            collect_ty_abi(inner, seen, out);
        }
        Ty::Lazy(inner) => {
            collect_ty_abi(inner, seen, out);
            push_abi_ty(ty, seen, out);
        }
        Ty::Map(key, value) => {
            collect_ty_abi(key, seen, out);
            collect_ty_abi(value, seen, out);
        }
        Ty::Tuple(items) => {
            push_abi_ty(ty, seen, out);
            for item in items {
                collect_ty_abi(item, seen, out);
            }
        }
        Ty::Func { params, ret } => {
            for param in params {
                collect_ty_abi(param, seen, out);
            }
            collect_ty_abi(ret, seen, out);
        }
        Ty::Named(_, args) => {
            for arg in args {
                collect_ty_abi(arg, seen, out);
            }
        }
        _ => {}
    }
}

fn push_abi_ty(ty: &Ty, seen: &mut HashSet<String>, out: &mut Vec<Ty>) {
    let name = ty_to_c(ty);
    if seen.insert(name) {
        out.push(ty.clone());
    }
}

fn c_bool(value: bool) -> String {
    if value {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

fn c_default_value(ty: &Ty) -> String {
    match ty {
        Ty::String => "ORI_STR(\"\")".to_string(),
        Ty::List(inner) => format!("ori_list_new(sizeof({}))", ty_to_c(inner)),
        Ty::Bool => "false".to_string(),
        Ty::Float | Ty::Float32 | Ty::Float64 => "0.0".to_string(),
        Ty::Void | Ty::Never => "((void)0)".to_string(),
        Ty::Optional(_) | Ty::Result(_, _) | Ty::Tuple(_) | Ty::Named(_, _) => {
            format!("(({}){{0}})", ty_to_c(ty))
        }
        _ => "0".to_string(),
    }
}

fn is_map_runtime_symbol(name: &str) -> bool {
    matches!(
        name,
        "ori_map_set"
            | "ori_map_get"
            | "ori_map_try_get"
            | "ori_map_contains"
            | "ori_map_remove"
            | "ori_map_try_remove"
    )
}

fn is_set_runtime_symbol(name: &str) -> bool {
    matches!(
        name,
        "ori_set_add" | "ori_set_contains" | "ori_set_remove" | "ori_set_try_remove"
    )
}

fn ty_to_c(ty: &Ty) -> String {
    match ty {
        Ty::Bool => "bool".into(),
        Ty::Int => "int64_t".into(),
        Ty::Int8 => "int8_t".into(),
        Ty::Int16 => "int16_t".into(),
        Ty::Int32 => "int32_t".into(),
        Ty::Int64 => "int64_t".into(),
        Ty::U8 => "uint8_t".into(),
        Ty::U16 => "uint16_t".into(),
        Ty::U32 => "uint32_t".into(),
        Ty::U64 => "uint64_t".into(),
        Ty::Float | Ty::Float64 => "double".into(),
        Ty::Float32 => "float".into(),
        Ty::String => "ori_string_t".into(),
        Ty::Bytes => "uint8_t*".into(),
        Ty::Void => "void".into(),
        Ty::Never => "void".into(),
        Ty::Optional(t) => format!("ori_opt_{}_t", ty_tag(t)),
        Ty::Result(ok, err) => format!("ori_result_{}_{}_t", ty_tag(ok), ty_tag(err)),
        Ty::List(_) => "ori_list_t".into(),
        Ty::Set(_) => "ori_set_t*".into(),
        Ty::Map(_, _) => "ori_map_t*".into(),
        Ty::Tuple(elems) => format!("ori_tuple_{}_t", tuple_ty_tag(elems)),
        Ty::Named(id, _) => def_c_name(*id),
        Ty::Any(_) => "ori_any_t".into(),
        Ty::Range(_) => "ori_range_t".into(),
        Ty::Func { .. } => "ori_closure_t*".into(),
        Ty::Lazy(inner) => format!("{}*", lazy_c_name(inner)),
        Ty::Future(_) | Ty::TaskJob(_) | Ty::Channel(_) | Ty::AtomicInt => "void*".into(),
        Ty::TaskJoinError | Ty::ChannelSendError | Ty::ChannelReceiveError => "void*".into(),
        _ => "void*".into(),
    }
}

fn abi_value_c_type(ty: &Ty) -> String {
    match ty {
        Ty::Void | Ty::Never => "ori_unit_t".into(),
        Ty::List(_) => "ori_list_t".into(),
        _ => ty_to_c(ty),
    }
}

fn c_arc_access(value: &str, ty: &Ty) -> Option<String> {
    match ty {
        Ty::String => Some(format!("(void*){}.data", value)),
        Ty::List(_) => Some(format!("(void*){}.data", value)),
        Ty::Bytes
        | Ty::Map(_, _)
        | Ty::Set(_)
        | Ty::Func { .. }
        | Ty::Lazy(_)
        | Ty::Future(_)
        | Ty::TaskJob(_)
        | Ty::Channel(_)
        | Ty::AtomicInt
        | Ty::TaskJoinError
        | Ty::ChannelSendError
        | Ty::ChannelReceiveError
        | Ty::Opaque { .. } => Some(format!("(void*){}", value)),
        Ty::Any(_) => Some(format!("(void*){}.obj", value)),
        _ => None,
    }
}

fn ty_tag(ty: &Ty) -> String {
    match ty {
        Ty::Bool => "bool".into(),
        Ty::Int => "i64".into(),
        Ty::Float => "f64".into(),
        Ty::String => "str".into(),
        Ty::Bytes => "bytes".into(),
        Ty::Optional(inner) => format!("opt_{}", ty_tag(inner)),
        Ty::Result(ok, err) => format!("result_{}_{}", ty_tag(ok), ty_tag(err)),
        Ty::List(inner) => format!("list_{}", ty_tag(inner)),
        Ty::Set(inner) => format!("set_{}", ty_tag(inner)),
        Ty::Map(k, v) => format!("map_{}_{}", ty_tag(k), ty_tag(v)),
        Ty::Lazy(inner) => format!("lazy_{}", ty_tag(inner)),
        Ty::Future(inner) => format!("future_{}", ty_tag(inner)),
        Ty::TaskJob(inner) => format!("task_job_{}", ty_tag(inner)),
        Ty::Channel(inner) => format!("channel_{}", ty_tag(inner)),
        Ty::AtomicInt => "atomic_int".into(),
        Ty::TaskJoinError => "task_join_error".into(),
        Ty::ChannelSendError => "channel_send_error".into(),
        Ty::ChannelReceiveError => "channel_receive_error".into(),
        Ty::Tuple(elems) => format!("tuple_{}", tuple_ty_tag(elems)),
        Ty::Named(id, _) => format!("def{}", id.0),
        _ => "any".into(),
    }
}

fn is_concurrency_runtime_symbol(name: &str) -> bool {
    name.starts_with("ori_task_")
        || name.starts_with("ori_channel_")
        || name.starts_with("ori_atomic_")
        || name.starts_with("ori_future_")
}

fn tuple_ty_tag(elems: &[Ty]) -> String {
    elems.iter().map(ty_tag).collect::<Vec<_>>().join("_")
}

fn lazy_c_name(inner: &Ty) -> String {
    format!("ori_lazy_{}_t", ty_tag(inner))
}

fn mangle(name: &str) -> String {
    let mut out = String::with_capacity(name.len() * 2);
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else if c == '.' {
            out.push_str("_dot_");
        } else {
            use std::fmt::Write;
            write!(&mut out, "_x{:02x}_", c as u32).unwrap();
        }
    }
    out
}

fn is_entry_main(module: &HirModule, f: &HirFunc) -> bool {
    let entry = if module.namespace.is_empty() {
        "main".to_string()
    } else {
        format!("{}.main", module.namespace)
    };
    f.params.is_empty() && f.name.as_str() == entry
}

fn binop_to_c(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

fn float_lit_to_c(value: f64) -> String {
    if value.is_nan() {
        "(0.0/0.0)".into()
    } else if value == f64::INFINITY {
        "(1.0/0.0)".into()
    } else if value == f64::NEG_INFINITY {
        "(-1.0/0.0)".into()
    } else {
        format!("{:.}", value)
    }
}

fn escape_c_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn pattern_cond(pat: &HirPattern, scrutinee: &str) -> String {
    match pat {
        HirPattern::Wildcard => "1".into(),
        HirPattern::BoolLit(b) => format!("{} == {}", scrutinee, if *b { "true" } else { "false" }),
        HirPattern::IntLit(n) => format!("{} == INT64_C({})", scrutinee, n),
        HirPattern::StrLit(s) => {
            let escaped = escape_c_str(s);
            format!("ori_string_eq({}, ORI_STR(\"{}\"))", scrutinee, escaped)
        }
        HirPattern::None_ => format!("!{}.has_value", scrutinee),
        HirPattern::Some_(_) => format!("{}.has_value", scrutinee),
        HirPattern::Ok_(_) => format!("{}.is_ok", scrutinee),
        HirPattern::Err_(_) => format!("!{}.is_ok", scrutinee),
        HirPattern::Variant {
            def_id, variant, ..
        } => {
            let type_name = def_c_name(*def_id);
            format!("{}.tag == {}__{}", scrutinee, type_name, mangle(variant))
        }
        HirPattern::Binding(_, _) => "1".into(), // always matches
        HirPattern::Tuple(_) => "1".into(),      // tuple always matches structurally
    }
}

fn emit_pattern_bindings(pat: &HirPattern, scrutinee: &str, out: &mut String, indent: usize) {
    let pad = "    ".repeat(indent);
    if let HirPattern::Binding(name, _) = pat {
        let _ = writeln!(out, "{}__auto_type {} = {};", pad, mangle(name), scrutinee);
    }
    if let HirPattern::Some_(inner) = pat {
        let inner_s = format!("{}.value", scrutinee);
        emit_pattern_bindings(inner, &inner_s, out, indent);
    }
    if let HirPattern::Ok_(inner) = pat {
        let inner_s = format!("{}.value.ok", scrutinee);
        emit_pattern_bindings(inner, &inner_s, out, indent);
    }
    if let HirPattern::Err_(inner) = pat {
        let inner_s = format!("{}.value.err", scrutinee);
        emit_pattern_bindings(inner, &inner_s, out, indent);
    }
    if let HirPattern::Variant {
        variant, fields, ..
    } = pat
    {
        for (fname, fpat) in fields {
            let field_s = format!(
                "{}.payload.{}.{}",
                scrutinee,
                mangle(variant),
                mangle(fname)
            );
            emit_pattern_bindings(fpat, &field_s, out, indent);
        }
    }
    if let HirPattern::Tuple(patterns) = pat {
        for (i, inner) in patterns.iter().enumerate() {
            let field_s = format!("{}._f{}", scrutinee, i);
            emit_pattern_bindings(inner, &field_s, out, indent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_diagnostics::Span;
    use ori_types::stdlib::stdlib_runtime_functions;
    use std::collections::HashSet;

    fn expr(kind: HirExprKind, ty: Ty) -> HirExpr {
        HirExpr {
            kind,
            ty,
            span: Span::DUMMY,
        }
    }

    fn empty_block() -> HirBlock {
        HirBlock {
            stmts: Vec::new(),
            span: Span::DUMMY,
        }
    }

    fn module_with_main(stmts: Vec<HirStmt>) -> HirModule {
        HirModule {
            namespace: "app.main".into(),
            structs: Vec::new(),
            enums: Vec::new(),
            traits: Vec::new(),
            trait_impls: Vec::new(),
            funcs: vec![HirFunc {
                def_id: DefId(1),
                name: "main".into(),
                params: Vec::new(),
                return_ty: Ty::Void,
                body: HirBlock {
                    stmts,
                    span: Span::DUMMY,
                },
                closure_captures: Vec::new(),
                is_public: false,
                is_async: false,
                is_mut: false,
                c_export_name: None,
                span: Span::DUMMY,
            }],
            consts: Vec::new(),
            externs: Vec::new(),
        }
    }

    #[test]
    fn c_backend_reports_unsupported_for_iterable() {
        let module = module_with_main(vec![HirStmt::For {
            binding: "item".into(),
            index_binding: None,
            elem_ty: Ty::Int,
            iterable: expr(HirExprKind::IntLit(1), Ty::Int),
            body: empty_block(),
            span: Span::DUMMY,
        }]);

        let err = CCodegen::new()
            .generate(&module)
            .expect_err("expected unsupported for-loop iterable error");
        assert!(
            err.contains("C backend does not support for-loop iterable type `int`"),
            "{err}"
        );
    }

    #[test]
    fn c_backend_reports_unsupported_lvalue_index_expression() {
        let module = module_with_main(vec![HirStmt::Assign {
            lvalue: HirLValue::Index {
                base: Box::new(HirLValue::Var("items".into())),
                index: Box::new(expr(HirExprKind::StrLit("bad".into()), Ty::String)),
            },
            value: expr(HirExprKind::IntLit(1), Ty::Int),
            span: Span::DUMMY,
        }]);

        let err = CCodegen::new()
            .generate(&module)
            .expect_err("expected unsupported lvalue index error");
        assert!(
            err.contains("C backend does not support `string` as lvalue index expression"),
            "{err}"
        );
    }

    #[test]
    fn c_backend_reports_loop_control_outside_loop() {
        let module = module_with_main(vec![
            HirStmt::Break(Span::DUMMY),
            HirStmt::Continue(Span::DUMMY),
        ]);

        let err = CCodegen::new()
            .generate(&module)
            .expect_err("expected invalid loop-control HIR error");
        assert!(
            err.contains("invalid HIR: `break` outside of loop reached C backend"),
            "{err}"
        );
        assert!(
            err.contains("invalid HIR: `continue` outside of loop reached C backend"),
            "{err}"
        );
    }

    #[test]
    fn c_backend_uses_bounds_checked_runtime_helpers_for_index_and_slice() {
        let list_ty = Ty::List(Box::new(Ty::Int));
        let module = module_with_main(vec![
            HirStmt::Let {
                name: "items".into(),
                ty: list_ty.clone(),
                mutable: false,
                value: expr(
                    HirExprKind::ListLit {
                        elem_ty: Ty::Int,
                        elements: vec![expr(HirExprKind::IntLit(1), Ty::Int)],
                    },
                    list_ty.clone(),
                ),
                span: Span::DUMMY,
            },
            HirStmt::Expr(expr(
                HirExprKind::Index {
                    object: Box::new(expr(HirExprKind::Var("items".into()), list_ty.clone())),
                    index: Box::new(expr(HirExprKind::IntLit(0), Ty::Int)),
                },
                Ty::Int,
            )),
            HirStmt::Expr(expr(
                HirExprKind::MethodCall {
                    receiver: Box::new(expr(HirExprKind::Var("items".into()), list_ty.clone())),
                    method: "__slice".into(),
                    args: vec![
                        expr(HirExprKind::IntLit(0), Ty::Int),
                        expr(HirExprKind::IntLit(1), Ty::Int),
                    ],
                },
                list_ty,
            )),
            HirStmt::Expr(expr(
                HirExprKind::MethodCall {
                    receiver: Box::new(expr(HirExprKind::StrLit("abcd".into()), Ty::String)),
                    method: "__slice".into(),
                    args: vec![
                        expr(HirExprKind::IntLit(1), Ty::Int),
                        expr(HirExprKind::IntLit(3), Ty::Int),
                    ],
                },
                Ty::String,
            )),
            HirStmt::Expr(expr(
                HirExprKind::Index {
                    object: Box::new(expr(HirExprKind::StrLit("xy".into()), Ty::String)),
                    index: Box::new(expr(HirExprKind::IntLit(1), Ty::Int)),
                },
                Ty::String,
            )),
        ]);

        let source = CCodegen::new()
            .generate(&module)
            .expect("C backend should generate indexed and sliced covered types");

        assert!(source.contains("ori_list_at(&items, (int64_t)"));
        assert!(source.contains("ori_list_slice(items,"));
        assert!(source.contains("ori_string_slice(ORI_STR(\"abcd\")"));
        assert!(source.contains("ori_string_get(ORI_STR(\"xy\")"));
        assert!(source.contains("ori list index out of bounds"));
        assert!(source.contains("ori list slice bounds out of range"));
        assert!(source.contains("ori string slice bounds out of range"));
    }

    #[test]
    fn c_backend_emits_arc_for_managed_locals_and_assignment() {
        let module = module_with_main(vec![
            HirStmt::Let {
                name: "greeting".into(),
                ty: Ty::String,
                mutable: true,
                value: expr(HirExprKind::StrLit("hello".into()), Ty::String),
                span: Span::DUMMY,
            },
            HirStmt::Assign {
                lvalue: HirLValue::Var("greeting".into()),
                value: expr(HirExprKind::StrLit("bye".into()), Ty::String),
                span: Span::DUMMY,
            },
        ]);

        let source = CCodegen::new()
            .generate(&module)
            .expect("C backend should generate managed local ARC calls");

        assert!(source.contains("typedef struct ori_arc_header"));
        assert!(!source.contains("static inline void ori_arc_retain(void* ptr) { (void)ptr; }"));
        assert!(source.contains("ori_arc_retain((void*)greeting.data);"));
        assert!(source.contains("void* _ori_tmp"));
        assert!(source.contains(" = (void*)greeting.data;"));
        assert!(source.contains("ori_arc_release(_ori_tmp"));
        assert!(source.contains("ori_arc_release((void*)greeting.data);"));
    }

    #[test]
    fn c_backend_emits_arc_edges_for_managed_closure_captures() {
        let capture = HirClosureCapture {
            name: "label".into(),
            ty: Ty::String,
        };
        let func_ty = Ty::Func {
            params: Vec::new(),
            ret: Box::new(Ty::String),
        };
        let module = HirModule {
            namespace: "app.main".into(),
            structs: Vec::new(),
            enums: Vec::new(),
            traits: Vec::new(),
            trait_impls: Vec::new(),
            funcs: vec![
                HirFunc {
                    def_id: DefId(1),
                    name: "main".into(),
                    params: Vec::new(),
                    return_ty: Ty::Void,
                    body: HirBlock {
                        stmts: vec![
                            HirStmt::Let {
                                name: "label".into(),
                                ty: Ty::String,
                                mutable: false,
                                value: expr(HirExprKind::StrLit("ok".into()), Ty::String),
                                span: Span::DUMMY,
                            },
                            HirStmt::Let {
                                name: "reader".into(),
                                ty: func_ty.clone(),
                                mutable: false,
                                value: expr(
                                    HirExprKind::Closure {
                                        func_name: "read_label".into(),
                                        captures: vec![capture.clone()],
                                    },
                                    func_ty,
                                ),
                                span: Span::DUMMY,
                            },
                        ],
                        span: Span::DUMMY,
                    },
                    closure_captures: Vec::new(),
                    is_public: false,
                    is_async: false,
                    is_mut: false,
                    c_export_name: None,
                    span: Span::DUMMY,
                },
                HirFunc {
                    def_id: DefId(2),
                    name: "read_label".into(),
                    params: Vec::new(),
                    return_ty: Ty::String,
                    body: HirBlock {
                        stmts: vec![HirStmt::Return(
                            Some(expr(HirExprKind::Var("label".into()), Ty::String)),
                            Span::DUMMY,
                        )],
                        span: Span::DUMMY,
                    },
                    closure_captures: vec![capture],
                    is_public: false,
                    is_async: false,
                    is_mut: false,
                    c_export_name: None,
                    span: Span::DUMMY,
                },
            ],
            consts: Vec::new(),
            externs: Vec::new(),
        };

        let source = CCodegen::new()
            .generate(&module)
            .expect("C backend should generate closure ARC edges");

        assert!(source.contains("(ori_closure_t*)ori_alloc(sizeof(ori_closure_t), 0)"));
        assert!(source.contains("ori_arc_register_edge((void*)_ori_tmp"));
        assert!(source.contains(".data"));
        assert!(source.contains("ori_arc_release((void*)_ori_tmp"));
    }

    #[test]
    fn c_backend_inline_runtime_has_arc_cycle_collector() {
        assert!(ORI_RUNTIME_H.contains("typedef struct ori_arc_edge"));
        assert!(ORI_RUNTIME_H.contains("static inline void ori_arc_register_edge"));
        assert!(ORI_RUNTIME_H.contains("static inline void ori_arc_update_edge"));
        assert!(ORI_RUNTIME_H.contains("static inline long long ori_arc_collect_cycles"));
        assert!(ORI_RUNTIME_H.contains("trial_count"));
        assert!(!ORI_RUNTIME_H.contains("ori_arc_collect_cycles(void) { return 0; }"));
    }

    #[test]
    fn c_backend_retains_managed_return_before_scope_cleanup() {
        let module = HirModule {
            namespace: "app.main".into(),
            structs: Vec::new(),
            enums: Vec::new(),
            traits: Vec::new(),
            trait_impls: Vec::new(),
            funcs: vec![HirFunc {
                def_id: DefId(1),
                name: "value".into(),
                params: Vec::new(),
                return_ty: Ty::String,
                body: HirBlock {
                    stmts: vec![
                        HirStmt::Let {
                            name: "label".into(),
                            ty: Ty::String,
                            mutable: false,
                            value: expr(HirExprKind::StrLit("ok".into()), Ty::String),
                            span: Span::DUMMY,
                        },
                        HirStmt::Return(
                            Some(expr(HirExprKind::Var("label".into()), Ty::String)),
                            Span::DUMMY,
                        ),
                    ],
                    span: Span::DUMMY,
                },
                closure_captures: Vec::new(),
                is_public: false,
                is_async: false,
                is_mut: false,
                c_export_name: None,
                span: Span::DUMMY,
            }],
            consts: Vec::new(),
            externs: Vec::new(),
        };

        let source = CCodegen::new()
            .generate(&module)
            .expect("C backend should retain managed return values");

        let retain_pos = source
            .find("ori_arc_retain((void*)_ori_tmp")
            .expect("return temp should be retained");
        let cleanup_pos = source
            .find("ori_arc_release((void*)label.data);")
            .expect("local managed value should be released");
        let return_pos = source.find("return _ori_tmp").expect("return temp missing");
        assert!(retain_pos < cleanup_pos);
        assert!(cleanup_pos < return_pos);
    }

    #[test]
    fn c_backend_inline_runtime_exports_manifest_symbols() {
        let mut checked = HashSet::new();
        let mut missing = Vec::new();
        for entry in stdlib_runtime_functions()
            .iter()
            .filter(|entry| entry.c_backend_runtime)
        {
            if checked.insert(entry.runtime_symbol)
                && !ORI_RUNTIME_H.contains(&format!("{}(", entry.runtime_symbol))
            {
                missing.push(entry.runtime_symbol);
            }
        }

        assert!(
            missing.is_empty(),
            "manifest runtime symbols missing from C backend inline runtime: {missing:#?}"
        );
    }
}
