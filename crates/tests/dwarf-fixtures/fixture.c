/* Source of the C fixtures; see `build.sh`. */
typedef struct {
    double x, y;
    unsigned char tag;
} point;

static unsigned long long counter;
static const unsigned short table[8] = {1, 1, 2, 3, 5, 8, 13, 21};

static inline point scale(point p, double by) {
    point r = {p.x * by, p.y * by, p.tag};
    return r;
}

__attribute__((noinline)) static double norm2(point p) {
    point s = scale(p, 2.0);
    return s.x * s.x + s.y * s.y;
}

__attribute__((noinline)) static unsigned long long mix(unsigned a, unsigned long long b, float c) {
    unsigned long long acc = b;
    for (unsigned i = 0; i < a; i++) {
        unsigned long long t = table[i % 8];
        if (t % 2 == 0) {
            acc *= t + 1;
        } else {
            acc ^= ((unsigned long long)c << (i % 64)) | ((unsigned long long)c >> ((64 - i) % 64));
        }
    }
    counter += acc & 0xff;
    return acc;
}

static unsigned fib(unsigned n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }

__attribute__((export_name("entry"))) unsigned long long entry(unsigned a, unsigned b) {
    point p = {a, b, 3};
    double n = norm2(p);
    unsigned long long m = mix(a, b, (float)n);
    unsigned long long f = 0;
    for (unsigned i = 0; i < a; i++) f += fib(i % 10);
    return m ^ f ^ counter;
}

__attribute__((export_name("select"))) long long select_op(int which, long long x, long long y) {
    switch (which) {
    case 0: return x + y;
    case 1: return x - y;
    case 2: return x * y;
    case 3: if (y != 0) return x / y; /* fall through */
    default: return fib((unsigned)which);
    }
}

/* Not called or exported: the linker tombstones its DWARF, or with
 * `--no-gc-sections` keeps it for walrus's gc to remove. */
__attribute__((noinline)) int unused_helper(int x) {
    int acc = 0;
    for (int i = 0; i < x; i++) acc += i * x;
    return acc;
}
